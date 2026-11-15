//! Grid movement and A* pathfinding following SRD 5.2.1 "Playing on a Grid":
//! - entering an adjacent square (orthogonal **or diagonal**) costs 1 square (5 ft);
//! - a square of Difficult Terrain costs 2 squares;
//! - diagonal movement can't cross the corner of a wall or other space-filling feature.
//!
//! Optional variant (configurable): alternating 5/10 ft diagonals, for groups that prefer it.

use crate::map::{Cell, GridMap};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DiagonalRule {
    /// SRD 5.2.1 default: every diagonal costs one square.
    #[default]
    Uniform,
    /// Variant: every second diagonal in a move costs two squares.
    Alternating,
}

/// Distance in squares between two cells for ranges and reach ("count squares ... by the shortest route").
pub fn grid_distance_squares(a: Cell, b: Cell) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

pub fn grid_distance_ft(a: Cell, b: Cell) -> i32 {
    grid_distance_squares(a, b) * crate::FEET_PER_SQUARE
}

/// Per-query movement context supplied by the session (creature positions, allies, etc).
pub trait MoveContext {
    /// Whether the mover may pass through this cell at all (beyond map collision).
    fn passable(&self, c: Cell) -> bool;
    /// Extra difficult terrain from occupants (e.g. a non-ally creature's space).
    fn difficult(&self, c: Cell) -> bool;
    /// Whether the mover may end its move here (can't end in another creature's space).
    fn can_end(&self, c: Cell) -> bool;
}

/// No creatures: only the map matters.
pub struct EmptyContext;
impl MoveContext for EmptyContext {
    fn passable(&self, _: Cell) -> bool {
        true
    }
    fn difficult(&self, _: Cell) -> bool {
        false
    }
    fn can_end(&self, _: Cell) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathStep {
    pub cell: Cell,
    /// Cost in feet to enter this cell.
    pub cost_ft: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    pub steps: Vec<PathStep>,
    pub total_ft: i32,
}

fn step_allowed(map: &GridMap, ctx: &dyn MoveContext, from: Cell, to: Cell) -> bool {
    if !map.in_bounds(to) || map.flags(to).blocks_movement || !ctx.passable(to) {
        return false;
    }
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    if dx != 0 && dy != 0 {
        // Corners: both orthogonal neighbours must be free of space-filling terrain.
        if map.flags((from.0 + dx, from.1)).blocks_movement || map.flags((from.0, from.1 + dy)).blocks_movement {
            return false;
        }
    }
    true
}

fn enter_cost_squares(map: &GridMap, ctx: &dyn MoveContext, to: Cell, diagonal: bool, diag_count: u32, rule: DiagonalRule) -> i32 {
    let base = if diagonal && rule == DiagonalRule::Alternating && diag_count % 2 == 1 { 2 } else { 1 };
    // "Difficult Terrain isn't cumulative": one extra square at most, however many sources.
    let difficult = map.flags(to).difficult || ctx.difficult(to);
    base + if difficult { 1 } else { 0 }
}

/// Finds the cheapest path from `start` to `goal`. Returns `None` if unreachable or the goal is not a
/// legal place to end. `max_ft` bounds the search (pass `i32::MAX` for exploration).
pub fn find_path(map: &GridMap, ctx: &dyn MoveContext, start: Cell, goal: Cell, rule: DiagonalRule, max_ft: i32) -> Option<Path> {
    if start == goal {
        return Some(Path { steps: vec![], total_ft: 0 });
    }
    if !map.in_bounds(goal) || map.flags(goal).blocks_movement || !ctx.passable(goal) || !ctx.can_end(goal) {
        return None;
    }
    // State includes diagonal parity for the Alternating variant.
    type State = (Cell, u32);
    let parity = |n: u32| if rule == DiagonalRule::Alternating { n % 2 } else { 0 };
    let h = |c: Cell| grid_distance_squares(c, goal);
    let mut open = BinaryHeap::new();
    let mut best: HashMap<State, i32> = HashMap::new();
    let mut came: HashMap<State, (State, i32)> = HashMap::new();
    let s0 = (start, 0);
    best.insert(s0, 0);
    open.push(Reverse((h(start), 0, start.0, start.1, 0u32)));
    let max_sq = if max_ft == i32::MAX { i32::MAX } else { max_ft / crate::FEET_PER_SQUARE };
    let mut found = None;
    while let Some(Reverse((_, g, x, y, par))) = open.pop() {
        let cur = ((x, y), par);
        if best.get(&cur).is_some_and(|b| *b < g) {
            continue;
        }
        if (x, y) == goal {
            found = Some(cur);
            break;
        }
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let to = (x + dx, y + dy);
                if !step_allowed(map, ctx, (x, y), to) {
                    continue;
                }
                let diagonal = dx != 0 && dy != 0;
                let cost = enter_cost_squares(map, ctx, to, diagonal, par, rule);
                let ng = g + cost;
                if ng > max_sq {
                    continue;
                }
                let npar = parity(par + u32::from(diagonal));
                let ns = (to, npar);
                if best.get(&ns).is_none_or(|b| ng < *b) {
                    best.insert(ns, ng);
                    came.insert(ns, (cur, cost));
                    open.push(Reverse((ng + h(to), ng, to.0, to.1, npar)));
                }
            }
        }
    }
    let mut cur = found?;
    let total = best[&cur];
    let mut steps = Vec::new();
    while cur != s0 {
        let (prev, cost) = came[&cur];
        steps.push(PathStep { cell: cur.0, cost_ft: cost * crate::FEET_PER_SQUARE });
        cur = prev;
    }
    steps.reverse();
    Some(Path { steps, total_ft: total * crate::FEET_PER_SQUARE })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::map::{AmbientLight, LayerKind};

    fn open_map(w: i32, h: i32) -> GridMap {
        GridMap::new(w, h, AmbientLight::Bright).unwrap()
    }

    #[test]
    fn diagonal_costs_one_square_by_default() {
        let m = open_map(10, 10);
        let p = find_path(&m, &EmptyContext, (0, 0), (4, 4), DiagonalRule::Uniform, i32::MAX).unwrap();
        assert_eq!((p.steps.len(), p.total_ft), (4, 20));
        assert_eq!(grid_distance_ft((0, 0), (4, 4)), 20);
    }

    #[test]
    fn alternating_variant_costs_more() {
        let m = open_map(10, 10);
        let p = find_path(&m, &EmptyContext, (0, 0), (4, 4), DiagonalRule::Alternating, i32::MAX).unwrap();
        assert_eq!(p.total_ft, 30); // 5+10+5+10
    }

    #[test]
    fn difficult_terrain_costs_double_and_is_not_cumulative() {
        let cat = Catalog::builtin();
        let mut m = open_map(5, 1);
        m.set_tile(LayerKind::Terrain, (1, 0), cat.get("mud")).unwrap();
        m.set_tile(LayerKind::FloorDecoration, (1, 0), cat.get("rubble")).unwrap();
        let p = find_path(&m, &EmptyContext, (0, 0), (2, 0), DiagonalRule::Uniform, i32::MAX).unwrap();
        assert_eq!(p.total_ft, 15);
        assert_eq!(p.steps[0].cost_ft, 10);
    }

    #[test]
    fn walls_block_and_corners_cannot_be_cut() {
        let cat = Catalog::builtin();
        let mut m = open_map(3, 3);
        m.set_tile(LayerKind::Walls, (1, 0), cat.get("stone_wall")).unwrap();
        // (0,0) -> (1,1) diagonally would cut the wall's corner? No: corner rule needs (1,0) or (0,1) blocked.
        let p = find_path(&m, &EmptyContext, (0, 0), (1, 1), DiagonalRule::Uniform, i32::MAX).unwrap();
        assert_eq!(p.total_ft, 10, "must go around the corner: down then right");
        assert!(find_path(&m, &EmptyContext, (0, 0), (1, 0), DiagonalRule::Uniform, i32::MAX).is_none());
    }

    #[test]
    fn respects_movement_budget() {
        let m = open_map(20, 1);
        assert!(find_path(&m, &EmptyContext, (0, 0), (6, 0), DiagonalRule::Uniform, 30).is_some());
        assert!(find_path(&m, &EmptyContext, (0, 0), (7, 0), DiagonalRule::Uniform, 30).is_none());
    }

    #[test]
    fn unreachable_enclosed_goal() {
        let cat = Catalog::builtin();
        let mut m = open_map(5, 5);
        for c in [(1, 1), (2, 1), (3, 1), (1, 2), (3, 2), (1, 3), (2, 3), (3, 3)] {
            m.set_tile(LayerKind::Walls, c, cat.get("stone_wall")).unwrap();
        }
        assert!(find_path(&m, &EmptyContext, (0, 0), (2, 2), DiagonalRule::Uniform, i32::MAX).is_none());
    }
}
