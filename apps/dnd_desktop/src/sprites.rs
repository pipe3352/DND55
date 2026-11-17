//! Original pixel-art sprites for characters and monsters (32×32, dragons 48×48).
//!
//! Humanoids come from one "paper doll" generator ([`Hum`]): head, headgear, face, body armour,
//! cape, wings, weapon and off-hand item. Beasts and other creatures have their own drawers.
//! [`creature`] picks a sprite from a key: `class:<icon>` or `mon:<creature type>:<name>` — the
//! monster's name and type choose the archetype and its colours, so unrelated monsters never share
//! one generic picture.

use crate::pixel::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Head {
    None,
    Helm { full: bool, plume: Option<C> },
    Hood,
    WizHat,
    Cap(Option<C>),
    Mitre,
    Crown,
    Horned,
    Antlers,
    Bandana,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Face {
    Normal,
    Skull,
    Snout,
    Tusks,
    Zombie,
    Glow,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Body {
    Plate,
    Cloth,
    Robe,
    Bare,
    Bones,
    Rags,
}
#[derive(Clone, Copy, PartialEq)]
pub enum W {
    None,
    Sword,
    Great,
    Axe,
    GreatAxe,
    Staff(C),
    Bow,
    Dagger,
    Mace,
    Spear,
    Lute,
    Club,
    Scythe,
    Orb(C),
    Claws,
}
#[derive(Clone, Copy, PartialEq)]
pub enum L {
    None,
    Shield(R, C),
    Dagger,
    Book(C),
    Orb(C),
}

#[derive(Clone, Copy)]
pub struct Hum {
    pub skin: R,
    pub hair: Option<R>,
    pub long_hair: bool,
    pub beard: Option<R>,
    pub head: Head,
    /// Colour of the headgear.
    pub hat: R,
    pub face: Face,
    pub ears: bool,
    pub horns: Option<R>,
    pub body: R,
    pub kind: Body,
    pub legs: R,
    pub cape: Option<R>,
    /// Wings: (colour, feathered).
    pub wings: Option<(R, bool)>,
    pub right: W,
    pub left: L,
    pub short: bool,
    pub eye: C,
    pub trim: C,
}

const FAIR: R = ramp(238, 194, 158);
const TAN: R = ramp(204, 150, 112);
const DARK_SKIN: R = ramp(140, 96, 70);
const BROWN_HAIR: R = ramp(110, 70, 44);
const BLOND: R = ramp(230, 196, 104);
const RED_HAIR: R = ramp(196, 84, 60);
const BLACK_HAIR: R = ramp(58, 50, 62);
const LEATHER: R = ramp(140, 96, 60);
const EYE: C = c(40, 30, 44);

impl Default for Hum {
    fn default() -> Self {
        Hum {
            skin: FAIR,
            hair: Some(BROWN_HAIR),
            long_hair: false,
            beard: None,
            head: Head::None,
            hat: STEEL,
            face: Face::Normal,
            ears: false,
            horns: None,
            body: LEATHER,
            kind: Body::Cloth,
            legs: ramp(84, 72, 80),
            cape: None,
            wings: None,
            right: W::None,
            left: L::None,
            short: false,
            eye: EYE,
            trim: GOLD.m,
        }
    }
}

/// Draws a humanoid. Feet stand on row 29 of a 32×32 canvas.
pub fn humanoid(s: &Hum) -> Pix {
    let mut p = Pix::new(32, 32);
    let bottom = 29;
    let (leg_h, body_h) = if s.short { (3, 7) } else { (5, 9) };
    let ly = bottom - leg_h + 1;
    let by = ly - body_h + 1;
    let hy = by - 9;
    let boot = c(70, 46, 36);

    // ---- behind: wings, cape
    if let Some((r, feather)) = s.wings {
        for side in [false, true] {
            let mut wg = Pix::new(32, 32);
            if feather {
                wg.sell(5.5, by as f32 + 2.0, 5.0, 8.0, r);
                for k in 0..4 {
                    wg.line(2 + k * 2, by + 1 + k, 3 + k * 2, by + 8, r.d);
                }
            } else {
                wg.poly(
                    &[
                        (11, by + 2),
                        (2, by - 5),
                        (0, by + 3),
                        (4, by + 3),
                        (3, by + 8),
                        (7, by + 5),
                        (11, by + 8),
                    ],
                    r.m,
                );
                wg.line(11, by + 2, 2, by - 5, r.l);
                wg.line(10, by + 3, 3, by + 7, r.d);
                wg.line(9, by + 3, 1, by + 2, r.d);
            }
            if side {
                wg = wg.mirror();
            }
            p.blit(&wg, 0, 0);
        }
    }
    if let Some(r) = s.cape {
        p.srect(9, by, 14, body_h + leg_h - 2, r);
        p.rect(10, by + body_h + leg_h - 3, 2, 1, CLEAR);
        p.rect(19, by + body_h + leg_h - 3, 2, 1, CLEAR);
    }
    // ---- legs
    if s.kind != Body::Robe {
        let lr = if s.kind == Body::Bones { BONE } else { s.legs };
        for x in [12, 17] {
            if s.kind == Body::Bones {
                p.rect(x + 1, ly, 1, leg_h, lr.m);
                p.rect(x, bottom, 3, 1, lr.l);
            } else {
                p.srect(x, ly, 3, leg_h, lr);
                p.rect(x - (x == 12) as i32, bottom - 1, 4, 2, boot);
            }
        }
    }
    // ---- body
    let arm = if matches!(s.kind, Body::Bare | Body::Bones) {
        s.skin
    } else {
        s.body
    };
    match s.kind {
        Body::Plate => {
            p.srect(11, by, 10, body_h, s.body);
            p.srect(9, by, 3, 3, s.body);
            p.srect(20, by, 3, 3, s.body);
            p.rect(13, by + 2, 4, 1, s.body.l);
            p.rect(11, by + body_h - 2, 10, 1, s.body.d);
            p.set(15, by + body_h - 2, s.trim);
            p.set(16, by + body_h - 2, s.trim);
        }
        Body::Cloth | Body::Rags => {
            p.srect(11, by, 10, body_h, s.body);
            p.set(15, by, s.skin.d);
            p.set(16, by, s.skin.d);
            p.rect(11, by + body_h - 2, 10, 1, c(70, 46, 36));
            p.set(16, by + body_h - 2, s.trim);
            p.line(12, by + 1, 19, by + body_h - 3, s.body.d);
            if s.kind == Body::Rags {
                for x in [11, 13, 16, 19] {
                    p.set(x, by + body_h - 1, CLEAR);
                }
                p.set(13, by + 3, s.skin.m);
                p.set(18, by + 5, s.skin.d);
            }
        }
        Body::Robe => {
            p.srect(11, by, 10, body_h, s.body);
            p.srect(10, by + body_h - 2, 12, leg_h + 2, s.body);
            p.rect(15, by + 1, 2, body_h + leg_h - 2, s.trim);
            p.rect(11, by + 4, 10, 1, s.body.d);
            p.rect(12, bottom, 3, 1, boot);
            p.rect(17, bottom, 3, 1, boot);
        }
        Body::Bare => {
            p.srect(11, by, 10, body_h, s.skin);
            p.rect(13, by + 3, 2, 1, s.skin.d);
            p.rect(17, by + 3, 2, 1, s.skin.d);
            p.set(16, by + 5, s.skin.d);
            p.srect(11, by + body_h - 3, 10, 3, s.legs);
            p.line(12, by, 19, by + body_h - 4, LEATHER.d);
        }
        Body::Bones => {
            p.rect(11, by, 10, body_h, c(46, 36, 48));
            p.rect(15, by, 2, body_h, BONE.m);
            for k in 0..3 {
                p.rect(12, by + 1 + k * 2, 8, 1, BONE.l);
            }
            p.rect(13, by + body_h - 1, 6, 1, BONE.m);
        }
    }
    // ---- arms and hands
    for (x, hx) in [(8, 8), (21, 22)] {
        p.srect(x, by + 1, 3, 6, arm);
        p.rect(hx, by + 7, 2, 2, s.skin.m);
    }
    // ---- head
    p.srect(10, hy, 12, 10, s.skin);
    for (x, y) in [(10, hy), (21, hy), (10, hy + 9), (21, hy + 9)] {
        p.set(x, y, CLEAR);
    }
    if s.ears {
        for (sx, d) in [(9, -1), (22, 1)] {
            p.rect(sx.min(sx + d), hy + 4, 2, 2, s.skin.m);
            p.set(sx + d * 2, hy + 3, s.skin.m);
            p.set(sx + d, hy + 3, s.skin.l);
        }
    }
    if let Some(h) = s.hair {
        p.rect(10, hy, 12, 3, h.m);
        p.rect(11, hy, 9, 1, h.l);
        p.rect(10, hy + 3, 1, 3, h.m);
        p.rect(21, hy + 3, 1, 3, h.d);
        p.set(10, hy, CLEAR);
        p.set(21, hy, CLEAR);
        if s.long_hair {
            p.rect(9, hy + 2, 2, 10, h.m);
            p.rect(21, hy + 2, 2, 10, h.d);
        }
    }
    let ey = hy + 5;
    match s.face {
        Face::Normal | Face::Tusks | Face::Zombie | Face::Glow => {
            for x in [13, 18] {
                p.rect(x, ey, 2, 2, s.eye);
                p.set(
                    x,
                    ey,
                    if s.face == Face::Glow {
                        WHITE
                    } else {
                        c(250, 250, 250)
                    },
                );
            }
            if s.face == Face::Tusks {
                p.set(12, hy + 8, WHITE);
                p.set(19, hy + 8, WHITE);
                p.rect(13, hy + 8, 6, 1, s.skin.d);
            }
            if s.face == Face::Zombie {
                p.rect(18, ey, 2, 2, WHITE);
                p.rect(13, hy + 8, 5, 1, c(60, 30, 40));
            }
        }
        Face::Skull => {
            for x in [12, 18] {
                p.rect(x, hy + 4, 3, 3, c(40, 30, 44));
                p.set(x + 1, hy + 5, s.eye);
            }
            p.set(16, hy + 7, c(40, 30, 44));
            for x in (12..20).step_by(2) {
                p.set(x, hy + 9, c(40, 30, 44));
            }
            p.set(10, hy + 8, CLEAR);
            p.set(21, hy + 8, CLEAR);
        }
        Face::Snout => {
            for x in [12, 19] {
                p.set(x, hy + 4, s.eye);
            }
            p.srect(
                13,
                hy + 5,
                6,
                4,
                ramp(s.skin.l[0], s.skin.l[1], s.skin.l[2]),
            );
            p.set(14, hy + 6, EYE);
            p.set(17, hy + 6, EYE);
            p.rect(14, hy + 8, 4, 1, s.skin.d);
        }
    }
    if let Some(b) = s.beard {
        p.rect(11, hy + 7, 10, 3, b.m);
        p.rect(13, hy + 10, 6, 2, b.m);
        p.rect(14, hy + 12, 4, 1, b.d);
        p.rect(14, hy + 8, 4, 1, s.skin.d);
    }
    if let Some(h) = s.horns {
        for (x, d) in [(11, -1), (20, 1)] {
            p.set(x, hy - 1, h.m);
            p.set(x + d, hy - 2, h.m);
            p.set(x + d, hy - 3, h.l);
            p.set(x + d * 2, hy - 4, h.l);
        }
    }
    // ---- headgear
    let hat = s.hat;
    match s.head {
        Head::None => {}
        Head::Helm { full, plume } => {
            p.srect(10, hy, 12, if full { 10 } else { 5 }, hat);
            p.set(10, hy, CLEAR);
            p.set(21, hy, CLEAR);
            if full {
                p.set(10, hy + 9, CLEAR);
                p.set(21, hy + 9, CLEAR);
                p.rect(12, hy + 4, 8, 2, c(30, 26, 36));
                p.rect(15, hy + 6, 2, 3, hat.d);
                p.set(13, hy + 4, s.eye);
                p.set(18, hy + 4, s.eye);
                p.rect(15, hy + 1, 2, 3, hat.l);
            } else {
                p.rect(10, hy + 5, 1, 4, hat.m);
                p.rect(21, hy + 5, 1, 4, hat.d);
                p.rect(10, hy + 4, 12, 1, hat.d);
                p.rect(15, hy + 4, 2, 3, hat.m);
            }
            if let Some(pc) = plume {
                p.rect(15, hy - 1, 3, 1, pc);
                p.rect(16, hy - 2, 4, 1, pc);
                p.rect(18, hy - 3, 3, 1, pc);
                p.set(21, hy - 2, pc);
            }
        }
        Head::Hood => {
            p.srect(9, hy - 1, 14, 5, hat);
            p.rect(9, hy + 4, 2, 7, hat.m);
            p.rect(21, hy + 4, 2, 7, hat.d);
            p.rect(11, hy + 3, 10, 1, hat.d);
            p.set(9, hy - 1, CLEAR);
            p.set(22, hy - 1, CLEAR);
            p.rect(15, hy - 2, 3, 1, hat.m);
        }
        Head::WizHat => {
            let ws = [12, 10, 9, 8, 6, 5, 4, 3, 2];
            for (i, w) in ws.iter().enumerate() {
                let y = hy + 1 - i as i32;
                let x = 16 - w / 2 + (i as i32) / 3;
                p.rect(x, y, *w, 1, hat.m);
                p.set(x, y, hat.l);
                p.set(x + w - 1, y, hat.d);
            }
            p.rect(7, hy + 2, 18, 2, hat.m);
            p.rect(7, hy + 3, 18, 1, hat.d);
            p.rect(11, hy + 1, 10, 1, s.trim);
            p.set(16, hy - 3, WHITE);
        }
        Head::Cap(feather) => {
            p.srect(10, hy - 1, 12, 4, hat);
            p.rect(7, hy + 2, 6, 1, hat.d);
            p.set(10, hy - 1, CLEAR);
            p.set(21, hy - 1, CLEAR);
            if let Some(f) = feather {
                p.line(20, hy, 24, hy - 5, f);
                p.line(21, hy, 25, hy - 4, f);
            }
        }
        Head::Mitre => {
            p.srect(11, hy - 4, 10, 7, hat);
            p.rect(13, hy - 5, 6, 1, hat.m);
            p.rect(15, hy - 6, 2, 1, hat.l);
            p.rect(15, hy - 3, 2, 5, s.trim);
            p.rect(13, hy - 1, 6, 1, s.trim);
            p.rect(11, hy + 2, 10, 1, s.trim);
        }
        Head::Crown => {
            p.rect(11, hy - 1, 10, 2, GOLD.m);
            p.rect(11, hy, 10, 1, GOLD.d);
            for x in [11, 15, 16, 20] {
                p.set(x, hy - 2, GOLD.l);
            }
            p.set(13, hy - 1, c(200, 50, 60));
            p.set(18, hy - 1, c(60, 120, 220));
        }
        Head::Horned => {
            p.srect(10, hy, 12, 5, hat);
            p.set(10, hy, CLEAR);
            p.set(21, hy, CLEAR);
            p.rect(15, hy + 4, 2, 3, hat.m);
            for (x, d) in [(9, -1), (22, 1)] {
                p.set(x, hy + 2, BONE.m);
                p.set(x + d, hy + 1, BONE.m);
                p.set(x + d * 2, hy, BONE.l);
                p.set(x + d * 2, hy - 1, BONE.l);
                p.set(x + d * 2, hy - 2, BONE.l);
                p.set(x + d, hy - 3, BONE.l);
            }
        }
        Head::Antlers => {
            p.rect(10, hy + 1, 12, 1, LEAF.m);
            p.set(12, hy, LEAF.l);
            p.set(17, hy, LEAF.l);
            for (x, d) in [(11, -1), (20, 1)] {
                p.line(x, hy, x + d * 2, hy - 5, WOOD.l);
                p.line(x + d, hy - 2, x + d * 4, hy - 4, WOOD.m);
                p.set(x + d * 3, hy - 6, WOOD.l);
            }
        }
        Head::Bandana => {
            p.rect(10, hy + 1, 12, 2, hat.m);
            p.rect(10, hy + 2, 12, 1, hat.d);
            p.set(22, hy + 3, hat.m);
            p.set(23, hy + 4, hat.d);
        }
    }
    // ---- off-hand
    match s.left {
        L::None => {}
        L::Shield(r, em) => {
            p.srect(3, by, 8, 10, r);
            p.set(3, by + 9, CLEAR);
            p.set(10, by + 9, CLEAR);
            p.set(3, by + 8, CLEAR);
            p.set(10, by + 8, CLEAR);
            p.rect(6, by + 2, 2, 6, em);
            p.rect(4, by + 4, 6, 1, em);
        }
        L::Dagger => {
            p.rect(7, by + 2, 1, 5, STEEL.l);
            p.rect(6, by + 7, 3, 1, GOLD.m);
        }
        L::Book(col) => {
            p.srect(4, by + 3, 6, 6, ramp(col[0], col[1], col[2]));
            p.rect(9, by + 4, 1, 4, WHITE);
            p.set(6, by + 5, GOLD.l);
        }
        L::Orb(col) => orb(&mut p, 6, by + 4, col),
    }
    // ---- weapon (viewer's right)
    let wd = WOOD;
    match s.right {
        W::None => {}
        W::Sword | W::Great => {
            let (top, w) = if s.right == W::Great {
                (by - 11, 3)
            } else {
                (by - 6, 2)
            };
            p.rect(24, top, w, by + 6 - top, STEEL.m);
            p.rect(24, top, 1, by + 6 - top, STEEL.l);
            p.set(24, top - 1, STEEL.l);
            p.rect(22, by + 6, w + 4, 1, GOLD.m);
            p.rect(24, by + 7, 1, 2, wd.d);
        }
        W::Axe | W::GreatAxe => {
            p.rect(24, by - 6, 1, body_h + 9, wd.m);
            p.srect(25, by - 6, 4, 6, STEEL);
            p.set(28, by - 6, CLEAR);
            p.set(28, by - 1, CLEAR);
            if s.right == W::GreatAxe {
                p.srect(20, by - 6, 4, 6, STEEL);
                p.set(20, by - 6, CLEAR);
                p.set(20, by - 1, CLEAR);
            }
        }
        W::Staff(col) => {
            p.rect(25, by - 7, 1, bottom - by + 8, wd.m);
            p.set(24, by - 8, wd.l);
            p.set(26, by - 8, wd.l);
            orb(&mut p, 25, by - 10, col);
        }
        W::Bow => {
            for (i, x) in [24, 25, 26, 27, 27, 27, 27, 27, 27, 27, 27, 26, 25, 24]
                .iter()
                .enumerate()
            {
                p.set(*x, by - 4 + i as i32, wd.m);
            }
            p.rect(23, by - 3, 1, 12, c(220, 220, 210));
            p.rect(18, by + 3, 8, 1, wd.l);
            p.set(26, by + 3, STEEL.l);
        }
        W::Dagger => {
            p.rect(24, by + 1, 1, 6, STEEL.l);
            p.rect(23, by + 7, 3, 1, GOLD.m);
        }
        W::Mace => {
            p.rect(24, by - 2, 1, 11, wd.m);
            p.srect(23, by - 6, 4, 4, STEEL);
            for (x, y) in [(22, by - 5), (27, by - 4), (24, by - 7), (25, by - 2)] {
                p.set(x, y, STEEL.l);
            }
        }
        W::Spear => {
            p.rect(25, by - 10, 1, bottom - by + 11, wd.m);
            p.rect(25, by - 14, 1, 4, STEEL.l);
            p.rect(24, by - 11, 3, 1, STEEL.m);
        }
        W::Lute => {
            p.line(20, by + 4, 26, by - 3, wd.d);
            p.sell(18.5, by as f32 + 6.5, 3.6, 3.4, WOOD);
            p.set(18, by + 6, EYE);
            p.line(19, by + 5, 25, by - 2, c(230, 220, 190));
            p.rect(26, by - 4, 2, 2, wd.l);
        }
        W::Club => {
            p.srect(23, by - 6, 4, 8, WOOD_D);
            p.rect(24, by + 2, 2, 6, WOOD_D.m);
            p.set(23, by - 4, STEEL.l);
            p.set(26, by - 2, STEEL.l);
        }
        W::Scythe => {
            p.rect(25, by - 10, 1, bottom - by + 11, wd.m);
            p.rect(17, by - 12, 9, 1, STEEL.l);
            p.rect(15, by - 11, 5, 1, STEEL.m);
            p.rect(14, by - 10, 3, 1, STEEL.d);
        }
        W::Orb(col) => {
            orb(&mut p, 25, by + 3, col);
            p.set(27, by, col);
            p.set(23, by - 1, col);
        }
        W::Claws => {
            for hx in [7, 8, 9, 22, 23, 24] {
                p.set(hx, by + 9, WHITE);
            }
        }
    }
    p.outline();
    p.ground_shadow(16.0, 30.0, 9.0, 1.9);
    p
}

fn orb(p: &mut Pix, x: i32, y: i32, col: C) {
    p.rect(x - 1, y - 1, 3, 3, col);
    p.set(x - 1, y - 1, WHITE);
    p.set(x + 1, y + 1, [col[0] / 2, col[1] / 2, col[2] / 2, 255]);
}

// ------------------------------------------------------------------ beasts and other shapes

#[derive(Clone, Copy)]
pub struct Quad {
    pub r: R,
    pub bulk: f32,
    pub pointed_ears: bool,
    pub mane: Option<R>,
    pub bushy_tail: bool,
    pub tusks: bool,
    pub stripes: Option<C>,
    pub horns: bool,
    pub wings: Option<R>,
    pub eye: C,
    pub long_snout: bool,
}
impl Quad {
    fn new(r: R) -> Self {
        Quad {
            r,
            bulk: 1.0,
            pointed_ears: true,
            mane: None,
            bushy_tail: true,
            tusks: false,
            stripes: None,
            horns: false,
            wings: None,
            eye: c(250, 220, 80),
            long_snout: true,
        }
    }
}

/// Side-view four-legged beast facing left.
pub fn quadruped(q: &Quad) -> Pix {
    let mut p = Pix::new(32, 32);
    let r = q.r;
    let b = q.bulk;
    if let Some(w) = q.wings {
        p.poly(
            &[
                (14, 15),
                (10, 3),
                (17, 7),
                (22, 2),
                (24, 9),
                (29, 6),
                (26, 15),
            ],
            w.m,
        );
        p.line(14, 15, 10, 3, w.l);
        p.line(18, 14, 22, 2, w.d);
        p.line(23, 14, 29, 6, w.d);
    }
    // tail
    if q.bushy_tail {
        p.sell(27.0, 14.0, 3.0, 4.5, r);
        p.set(28, 10, r.l);
    } else {
        p.line(26, 17, 30, 12, r.d);
        p.line(30, 12, 30, 9, r.d);
    }
    // legs
    for (x, far) in [(9, false), (13, true), (20, true), (24, false)] {
        let col = if far { r.d } else { r.m };
        p.rect(x, 21, 3, 8, col);
        p.rect(x - 1, 28, 4, 2, if far { r.d } else { r.l });
    }
    // body, neck, head
    p.sell(17.0, 18.0, 9.5 * b.min(1.15), 5.0 * b, r);
    p.rect(10, 20, 14, 1, r.d);
    if let Some(m) = q.mane {
        p.sell(10.0, 14.5, 5.5, 6.5, m);
    }
    p.sell(7.5, 13.5, 4.8, 4.4, r);
    if q.long_snout {
        p.srect(1, 14, 5, 4, r);
        p.set(1, 14, EYE);
        p.rect(2, 17, 3, 1, r.d);
    } else {
        p.set(3, 15, EYE);
    }
    p.rect(6, 12, 2, 2, q.eye);
    p.set(6, 12, WHITE);
    if q.pointed_ears {
        p.rect(7, 7, 2, 3, r.m);
        p.set(7, 6, r.l);
        p.rect(10, 8, 2, 2, r.d);
    } else {
        p.rect(8, 8, 3, 2, r.m);
    }
    if q.tusks {
        p.set(3, 18, WHITE);
        p.set(2, 19, WHITE);
        p.set(5, 18, WHITE);
    }
    if q.horns {
        p.line(8, 8, 5, 4, BONE.l);
        p.line(10, 8, 12, 4, BONE.m);
    }
    if let Some(s) = q.stripes {
        for x in [12, 15, 18, 21] {
            p.rect(x, 14, 1, 5, s);
        }
    }
    p.outline();
    p.ground_shadow(16.0, 30.0, 12.0, 1.9);
    p
}

fn slime(r: R, cube: bool) -> Pix {
    let mut p = Pix::new(32, 32);
    if cube {
        p.srect(6, 8, 20, 21, r);
        p.rect(9, 11, 14, 15, r.l);
        p.rect(12, 16, 5, 1, BONE.m);
        p.rect(17, 20, 1, 4, BONE.m);
        p.sell(20.0, 15.0, 2.0, 2.0, BONE);
        p.set(19, 15, EYE);
        p.set(21, 15, EYE);
    } else {
        p.sell(16.0, 22.0, 12.0, 8.0, r);
        p.sell(16.0, 18.0, 8.5, 7.0, r);
        p.rect(4, 26, 24, 4, r.m);
        p.rect(4, 29, 24, 1, r.d);
        p.rect(4, 26, 1, 4, CLEAR);
        p.rect(27, 26, 1, 4, CLEAR);
        p.rect(10, 14, 4, 2, r.l);
        p.set(9, 16, WHITE);
        p.set(10, 15, WHITE);
        for x in [12, 19] {
            p.rect(x, 20, 2, 3, EYE);
            p.set(x, 20, WHITE);
        }
        p.set(24, 23, r.l);
        p.set(7, 24, r.d);
    }
    p.outline();
    p.ground_shadow(16.0, 30.0, 12.0, 1.8);
    p
}

fn spider(r: R, scorpion: bool) -> Pix {
    let mut p = Pix::new(32, 32);
    for (i, (kx, ky)) in [(-9i32, -8i32), (-11, -3), (-11, 3), (-8, 8)]
        .into_iter()
        .enumerate()
    {
        for s in [-1, 1] {
            let (bx, by) = (16 + s * 4, 17 + i as i32 - 1);
            let (mx, my) = (16 + s * (kx.abs() - 2), 17 + ky - 3);
            p.line(bx, by, mx, my, r.m);
            p.line(mx, my, 16 + s * kx.abs(), 17 + ky + 3, r.d);
        }
    }
    p.sell(16.0, 20.0, 6.5, 6.0, r);
    p.sell(16.0, 12.5, 4.0, 3.5, r);
    for x in [14, 17] {
        p.set(x, 11, c(230, 50, 50));
        p.set(x, 13, c(230, 50, 50));
    }
    p.rect(14, 19, 4, 1, r.l);
    p.set(15, 22, c(200, 60, 50));
    p.set(16, 21, c(200, 60, 50));
    if scorpion {
        p.line(16, 25, 22, 28, r.m);
        p.line(22, 28, 26, 22, r.m);
        p.line(26, 22, 23, 17, r.l);
        p.set(22, 16, WHITE);
        p.srect(8, 5, 4, 4, r);
        p.srect(20, 5, 4, 4, r);
    }
    p.outline();
    p.ground_shadow(16.0, 28.5, 11.0, 2.5);
    p
}

fn flyer(r: R, bird: bool) -> Pix {
    let mut p = Pix::new(32, 32);
    for side in [false, true] {
        let mut w = Pix::new(32, 32);
        if bird {
            w.poly(
                &[
                    (14, 13),
                    (2, 7),
                    (4, 12),
                    (2, 13),
                    (6, 16),
                    (5, 18),
                    (14, 18),
                ],
                r.m,
            );
            w.line(14, 13, 2, 7, r.l);
            w.line(6, 16, 13, 16, r.d);
        } else {
            w.poly(
                &[
                    (14, 13),
                    (4, 5),
                    (1, 13),
                    (4, 12),
                    (5, 17),
                    (8, 14),
                    (10, 19),
                    (14, 17),
                ],
                r.m,
            );
            w.line(14, 13, 4, 5, r.l);
            w.line(13, 14, 5, 16, r.d);
            w.line(12, 14, 2, 12, r.d);
        }
        p.blit(&if side { w.mirror() } else { w }, 0, 0);
    }
    p.sell(16.0, 15.0, 3.6, 5.0, r);
    p.sell(16.0, 10.0, 3.0, 2.8, r);
    if bird {
        p.rect(15, 12, 2, 2, GOLD.m);
        p.rect(14, 20, 4, 2, r.d);
    } else {
        p.rect(13, 6, 2, 3, r.m);
        p.rect(17, 6, 2, 3, r.d);
        p.set(15, 12, WHITE);
        p.set(17, 12, WHITE);
    }
    p.set(14, 9, c(240, 80, 60));
    p.set(17, 9, c(240, 80, 60));
    p.outline();
    p.ground_shadow(16.0, 29.0, 7.0, 1.6);
    p
}

fn snake(r: R, belly: C) -> Pix {
    let mut p = Pix::new(32, 32);
    p.sell(16.0, 24.0, 11.0, 5.0, r);
    p.sell(16.0, 23.0, 6.5, 2.2, ramp(r.d[0], r.d[1], r.d[2]));
    p.sell(17.0, 19.0, 8.0, 4.0, r);
    p.srect(18, 8, 5, 12, r);
    p.rect(19, 12, 3, 8, belly);
    p.sell(18.5, 7.5, 4.5, 3.5, r);
    p.rect(16, 6, 2, 2, c(250, 220, 80));
    p.set(16, 6, EYE);
    p.line(14, 9, 11, 10, c(220, 50, 60));
    p.set(10, 9, c(220, 50, 60));
    for x in [9, 13, 20, 24] {
        p.set(x, 23, r.l);
        p.set(x + 1, 26, r.d);
    }
    p.outline();
    p.ground_shadow(16.0, 29.5, 12.0, 1.8);
    p
}

fn dragon(r: R, belly: R) -> Pix {
    let mut p = Pix::new(48, 48);
    // far wing, tail
    p.poly(
        &[
            (26, 22),
            (30, 3),
            (36, 9),
            (42, 4),
            (43, 13),
            (47, 12),
            (42, 24),
        ],
        r.d,
    );
    p.line(26, 22, 30, 3, r.m);
    p.line(33, 21, 42, 4, r.m);
    for (i, (x, y)) in [(36, 32), (41, 34), (44, 31), (46, 27)].iter().enumerate() {
        p.sell(
            *x as f32,
            *y as f32,
            4.0 - i as f32 * 0.7,
            3.0 - i as f32 * 0.5,
            r,
        );
    }
    p.poly(&[(45, 26), (47, 20), (43, 23)], r.l);
    // legs
    for (x, far) in [(15, false), (20, true), (30, true), (34, false)] {
        p.srect(
            x,
            33,
            5,
            9,
            if far { ramp(r.d[0], r.d[1], r.d[2]) } else { r },
        );
        p.rect(x - 1, 41, 7, 2, r.m);
        for k in 0..3 {
            p.set(x - 1 + k * 3, 43, WHITE);
        }
    }
    // body + belly
    p.sell(26.0, 29.0, 13.0, 8.0, r);
    p.sell(24.0, 33.0, 9.0, 3.5, belly);
    // neck and head
    p.poly(&[(14, 26), (10, 14), (17, 12), (22, 24)], r.m);
    p.line(14, 26, 10, 14, r.l);
    p.line(13, 24, 16, 14, belly.m);
    p.sell(10.5, 11.5, 6.0, 4.5, r);
    p.srect(1, 11, 8, 5, r);
    p.rect(1, 15, 8, 1, belly.m);
    p.set(2, 11, EYE);
    for x in [2, 4, 6] {
        p.set(x, 16, WHITE);
    }
    p.rect(9, 9, 2, 2, c(255, 210, 60));
    p.set(10, 9, EYE);
    p.line(13, 8, 18, 3, BONE.l);
    p.line(15, 9, 20, 6, BONE.m);
    for (x, y) in [(13, 15), (15, 18), (17, 21)] {
        p.set(x + 3, y - 3, r.l);
    }
    // near wing
    p.poly(
        &[
            (24, 24),
            (18, 4),
            (26, 9),
            (33, 2),
            (35, 11),
            (41, 8),
            (36, 25),
        ],
        r.m,
    );
    p.line(24, 24, 18, 4, r.l);
    p.line(28, 23, 33, 2, r.d);
    p.line(33, 24, 41, 8, r.d);
    p.line(18, 4, 26, 9, r.l);
    p.outline();
    p.ground_shadow(25.0, 44.5, 19.0, 2.6);
    p
}

fn flame_elemental() -> Pix {
    let mut p = Pix::new(32, 32);
    for (pts, col) in [
        (
            vec![
                (16, 1),
                (22, 9),
                (26, 6),
                (27, 18),
                (24, 27),
                (16, 30),
                (8, 27),
                (5, 18),
                (8, 8),
                (11, 12),
            ],
            c(214, 60, 30),
        ),
        (
            vec![
                (16, 6),
                (21, 13),
                (23, 20),
                (20, 27),
                (16, 29),
                (11, 27),
                (9, 19),
                (12, 12),
            ],
            c(250, 140, 40),
        ),
        (
            vec![(16, 13), (19, 19), (18, 26), (14, 26), (12, 20)],
            c(255, 226, 110),
        ),
    ] {
        p.poly(&pts, col);
    }
    for x in [13, 18] {
        p.rect(x, 17, 2, 3, c(90, 20, 20));
    }
    p.rect(14, 23, 5, 1, c(90, 20, 20));
    p.set(6, 4, c(255, 200, 80));
    p.set(25, 2, c(255, 160, 60));
    p.outline();
    p.ground_shadow(16.0, 30.5, 9.0, 1.4);
    p
}

fn blob_elemental(r: R, swirl: bool) -> Pix {
    let mut p = Pix::new(32, 32);
    if swirl {
        for (i, ry) in [(0, 3.0f32), (1, 4.0), (2, 5.0), (3, 6.0)] {
            let y = 26.0 - i as f32 * 6.0;
            p.sell(16.0 + (i as f32 - 1.5) * 1.5, y, 4.0 + ry * 1.3, 3.2, r);
            p.rect(10 + i, y as i32 - 1, 8, 1, WHITE);
        }
    } else {
        p.sell(16.0, 21.0, 9.0, 9.0, r);
        p.sell(16.0, 11.0, 6.5, 6.0, r);
        p.sell(7.0, 18.0, 3.5, 6.0, r);
        p.sell(25.0, 18.0, 3.5, 6.0, r);
        p.rect(11, 6, 5, 1, WHITE);
        p.rect(8, 27, 16, 1, WHITE);
    }
    for x in [13, 18] {
        p.rect(x, 10, 2, 2, WHITE);
    }
    p.outline();
    p.ground_shadow(16.0, 30.5, 10.0, 1.5);
    p
}

fn rock_elemental(r: R, eye: C) -> Pix {
    let mut p = Pix::new(32, 32);
    p.srect(9, 21, 5, 9, r);
    p.srect(18, 21, 5, 9, r);
    p.srect(7, 10, 18, 13, r);
    p.srect(2, 11, 6, 12, r);
    p.srect(24, 11, 6, 12, r);
    p.srect(11, 3, 10, 8, r);
    p.rect(13, 6, 2, 2, eye);
    p.rect(17, 6, 2, 2, eye);
    p.line(10, 13, 14, 17, r.d);
    p.line(14, 17, 13, 21, r.d);
    p.line(20, 12, 22, 16, r.d);
    p.set(9, 12, r.l);
    p.rect(4, 22, 3, 2, r.d);
    p.rect(25, 22, 3, 2, r.d);
    p.outline();
    p.ground_shadow(16.0, 30.5, 11.0, 1.5);
    p
}

fn ghost(r: R, eye: C, alpha: u8) -> Pix {
    let mut p = Pix::new(32, 32);
    p.sell(16.0, 11.0, 7.5, 7.5, r);
    p.srect(9, 11, 14, 14, r);
    for (i, x) in (9..23).enumerate() {
        let d = [3, 2, 1, 2, 3, 4, 3][i % 7];
        p.rect(x, 25, 1, d, r.m);
    }
    p.srect(4, 14, 5, 4, r);
    p.srect(23, 14, 5, 4, r);
    p.sell(16.0, 11.0, 5.0, 5.0, ramp(30, 26, 40));
    for x in [13, 17] {
        p.rect(x, 10, 2, 2, eye);
    }
    p.outline();
    p.tint([1.0, 1.0, 1.0], alpha);
    p.ground_shadow(16.0, 30.5, 7.0, 1.3);
    p
}

fn eye_beast(r: R) -> Pix {
    let mut p = Pix::new(32, 32);
    for (i, x) in [6, 11, 16, 21, 26].iter().enumerate() {
        let sway = [-2, 1, 0, -1, 2][i];
        p.line(*x, 18, x + sway, 27, r.m);
        p.line(x + sway, 27, x + sway * 2, 30, r.d);
        p.set(x + sway * 2, 30, r.l);
    }
    p.sell(16.0, 13.0, 11.0, 9.5, r);
    p.sell(16.0, 12.0, 6.0, 5.5, BONE);
    p.sell(16.0, 12.0, 3.2, 3.6, ramp(210, 150, 40));
    p.rect(15, 10, 2, 5, EYE);
    p.set(14, 9, WHITE);
    p.rect(11, 19, 10, 1, EYE);
    for x in [12, 15, 18] {
        p.set(x, 20, WHITE);
    }
    p.outline();
    p.ground_shadow(16.0, 30.5, 10.0, 1.3);
    p
}

fn plant(r: R, fungus: bool) -> Pix {
    let mut p = Pix::new(32, 32);
    if fungus {
        p.srect(12, 16, 8, 13, BONE);
        p.sell(16.0, 11.0, 12.0, 7.5, r);
        p.rect(4, 13, 24, 5, CLEAR);
        p.sell(16.0, 12.0, 12.0, 4.0, r);
        for (x, y) in [(9, 8), (15, 6), (21, 9), (12, 11), (19, 12)] {
            p.rect(x, y, 2, 2, WHITE);
        }
        p.rect(13, 20, 2, 2, EYE);
        p.rect(17, 20, 2, 2, EYE);
        p.rect(14, 24, 4, 1, EYE);
    } else {
        p.srect(13, 18, 6, 11, WOOD_D);
        p.line(13, 22, 5, 16, WOOD_D.m);
        p.line(18, 22, 27, 15, WOOD_D.m);
        p.rect(10, 28, 12, 2, WOOD_D.d);
        for (x, y, rr) in [
            (10.0, 11.0, 6.0),
            (21.0, 10.0, 6.5),
            (16.0, 6.5, 6.0),
            (16.0, 13.0, 7.0),
            (5.0, 15.0, 3.0),
            (27.0, 14.0, 3.0),
        ] {
            p.sell(x, y, rr, rr * 0.85, r);
        }
        p.rect(13, 20, 2, 2, c(255, 220, 90));
        p.rect(17, 20, 2, 2, c(255, 220, 90));
        p.rect(14, 24, 4, 2, EYE);
    }
    p.outline();
    p.ground_shadow(16.0, 30.5, 10.0, 1.4);
    p
}

fn mimic() -> Pix {
    let mut p = Pix::new(32, 32);
    p.srect(5, 17, 22, 12, WOOD);
    p.srect(5, 5, 22, 9, WOOD);
    p.rect(5, 13, 22, 4, c(120, 30, 50));
    for x in (6..26).step_by(3) {
        p.rect(x, 13, 2, 2, WHITE);
        p.rect(x + 1, 15, 2, 2, WHITE);
    }
    p.poly(
        &[(14, 15), (20, 15), (24, 24), (20, 26), (16, 22)],
        c(220, 90, 110),
    );
    for x in [8, 22] {
        p.rect(x, 5, 2, 9, IRON.m);
        p.rect(x, 17, 2, 12, IRON.m);
    }
    p.rect(11, 8, 3, 3, c(255, 220, 60));
    p.rect(18, 8, 3, 3, c(255, 220, 60));
    p.set(12, 9, EYE);
    p.set(19, 9, EYE);
    p.outline();
    p.ground_shadow(16.0, 30.0, 12.0, 1.8);
    p
}

fn fish(r: R, shark: bool) -> Pix {
    let mut p = Pix::new(32, 32);
    p.poly(&[(24, 17), (31, 10), (29, 17), (31, 24)], r.d);
    p.sell(14.0, 17.0, 11.5, if shark { 5.5 } else { 7.0 }, r);
    p.sell(13.0, 20.0, 9.0, 2.5, BONE);
    p.poly(&[(13, 12), (18, 4), (20, 12)], r.m);
    p.poly(&[(13, 21), (17, 27), (19, 21)], r.d);
    p.rect(6, 14, 2, 2, WHITE);
    p.set(6, 15, EYE);
    if shark {
        for x in [3, 5, 7] {
            p.set(x, 19, WHITE);
        }
    }
    p.line(11, 14, 11, 19, r.d);
    p.outline();
    p.ground_shadow(15.0, 29.5, 10.0, 1.5);
    p
}

// ------------------------------------------------------------------ lookup

fn hash(s: &str) -> u32 {
    s.bytes().fold(2_166_136_261u32, |h, b| {
        (h ^ b as u32).wrapping_mul(16_777_619)
    })
}

fn class_sprite(icon: &str) -> Hum {
    let d = Hum::default();
    let plate = Hum {
        kind: Body::Plate,
        body: STEEL,
        legs: IRON,
        ..d
    };
    match icon {
        "knight" => Hum {
            head: Head::Helm {
                full: true,
                plume: Some(c(200, 50, 50)),
            },
            right: W::Sword,
            left: L::Shield(ramp(170, 50, 50), GOLD.l),
            ..plate
        },
        "paladin" => Hum {
            head: Head::Helm {
                full: true,
                plume: None,
            },
            hat: ramp(210, 214, 224),
            body: ramp(210, 214, 224),
            trim: GOLD.l,
            cape: Some(ramp(60, 90, 170)),
            right: W::Mace,
            left: L::Shield(ramp(232, 232, 236), c(200, 50, 50)),
            eye: c(250, 220, 120),
            ..plate
        },
        "mage" => Hum {
            head: Head::WizHat,
            hat: ramp(70, 80, 190),
            hair: Some(BLOND),
            long_hair: true,
            kind: Body::Robe,
            body: ramp(70, 80, 190),
            right: W::Staff(c(90, 220, 240)),
            ..d
        },
        "archer" => Hum {
            head: Head::Cap(Some(c(220, 60, 50))),
            hat: ramp(80, 120, 60),
            body: ramp(90, 130, 70),
            right: W::Bow,
            hair: Some(BLOND),
            ..d
        },
        "rogue" => Hum {
            head: Head::Hood,
            hat: ramp(64, 58, 78),
            body: ramp(64, 58, 78),
            legs: ramp(50, 46, 60),
            cape: Some(ramp(50, 46, 60)),
            right: W::Dagger,
            left: L::Dagger,
            hair: None,
            ..d
        },
        "cleric" => Hum {
            head: Head::Mitre,
            hat: ramp(236, 232, 222),
            kind: Body::Robe,
            body: ramp(236, 232, 222),
            trim: GOLD.m,
            right: W::Mace,
            left: L::Book(c(170, 50, 50)),
            hair: Some(RED_HAIR),
            long_hair: true,
            ..d
        },
        "druid" => Hum {
            head: Head::Antlers,
            kind: Body::Robe,
            body: ramp(74, 110, 66),
            trim: c(150, 110, 60),
            right: W::Staff(c(120, 220, 90)),
            hair: Some(BROWN_HAIR),
            long_hair: true,
            beard: Some(BROWN_HAIR),
            skin: TAN,
            ..d
        },
        "barbarian" => Hum {
            head: Head::Horned,
            hat: IRON,
            kind: Body::Bare,
            skin: TAN,
            legs: ramp(120, 80, 50),
            hair: Some(RED_HAIR),
            long_hair: true,
            beard: Some(RED_HAIR),
            right: W::GreatAxe,
            ..d
        },
        "bard" => Hum {
            head: Head::Cap(Some(c(250, 240, 230))),
            hat: ramp(150, 50, 110),
            body: ramp(150, 50, 110),
            legs: ramp(60, 90, 140),
            trim: GOLD.l,
            right: W::Lute,
            hair: Some(BLACK_HAIR),
            ..d
        },
        "monk" => Hum {
            hair: None,
            kind: Body::Robe,
            body: ramp(220, 130, 50),
            trim: c(150, 60, 40),
            skin: TAN,
            right: W::None,
            head: Head::Bandana,
            hat: ramp(190, 50, 40),
            ..d
        },
        "ranger" => Hum {
            head: Head::Hood,
            hat: ramp(60, 96, 60),
            body: ramp(110, 84, 54),
            cape: Some(ramp(60, 96, 60)),
            right: W::Bow,
            left: L::Dagger,
            hair: None,
            skin: TAN,
            ..d
        },
        "sorcerer" => Hum {
            hair: Some(ramp(240, 240, 250)),
            long_hair: true,
            kind: Body::Robe,
            body: ramp(180, 50, 50),
            trim: GOLD.l,
            right: W::Orb(c(255, 150, 50)),
            left: L::Orb(c(255, 210, 90)),
            eye: c(220, 120, 30),
            ..d
        },
        "warlock" => Hum {
            head: Head::Hood,
            hat: ramp(90, 50, 130),
            kind: Body::Robe,
            body: ramp(60, 44, 90),
            trim: c(170, 90, 220),
            right: W::Orb(c(190, 100, 255)),
            left: L::Book(c(60, 40, 70)),
            hair: None,
            skin: ramp(214, 200, 210),
            eye: c(190, 100, 255),
            ..d
        },
        _ => Hum {
            head: Head::Helm {
                full: false,
                plume: None,
            },
            right: W::Sword,
            left: L::Shield(ramp(70, 100, 170), GOLD.l),
            hair: Some(BROWN_HAIR),
            ..plate
        },
    }
}

fn dragon_colors(n: &str, h: u32) -> (R, R) {
    let table: [(&str, R, R); 10] = [
        ("red", ramp(190, 44, 40), ramp(240, 180, 100)),
        ("blue", ramp(50, 100, 200), ramp(170, 210, 240)),
        ("green", ramp(60, 140, 70), ramp(190, 220, 130)),
        ("black", ramp(60, 56, 70), ramp(130, 120, 140)),
        ("white", ramp(216, 226, 236), ramp(170, 200, 230)),
        ("gold", ramp(226, 176, 60), ramp(250, 230, 150)),
        ("silver", ramp(170, 180, 196), ramp(226, 232, 240)),
        ("bronze", ramp(170, 120, 70), ramp(120, 190, 170)),
        ("brass", ramp(200, 160, 80), ramp(240, 220, 150)),
        ("copper", ramp(200, 110, 70), ramp(240, 190, 130)),
    ];
    table
        .iter()
        .find(|t| n.contains(t.0))
        .map(|t| (t.1, t.2))
        .unwrap_or_else(|| {
            let t = table[(h % 5) as usize];
            (t.1, t.2)
        })
}

fn monster(ty: &str, n: &str) -> Pix {
    let h = hash(n);
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    let d = Hum::default();
    let pick = |opts: &[W]| opts[(h % opts.len() as u32) as usize];
    let fur = |i: u32| {
        [
            ramp(120, 110, 104),
            ramp(140, 100, 64),
            ramp(90, 84, 90),
            ramp(196, 170, 120),
        ][(i % 4) as usize]
    };
    let goblin = ramp(112, 160, 70);
    let demon = ramp(186, 48, 44);
    // ---- dragons
    if has(&["dragon", "wyrm", "wyvern", "drake"]) && !has(&["dragonborn"]) {
        let (r, b) = dragon_colors(n, h);
        return dragon(r, b);
    }
    // ---- undead
    if has(&["skeleton"]) {
        return humanoid(&Hum {
            skin: BONE,
            hair: None,
            face: Face::Skull,
            kind: Body::Bones,
            eye: c(230, 60, 50),
            horns: has(&["minotaur"]).then_some(BONE),
            right: pick(&[W::Sword, W::Axe, W::Spear]),
            left: if h.is_multiple_of(2) {
                L::Shield(WOOD, IRON.l)
            } else {
                L::None
            },
            ..d
        });
    }
    if has(&["zombie", "ghoul", "ghast"]) {
        let ghoul = !has(&["zombie"]);
        return humanoid(&Hum {
            skin: if ghoul {
                ramp(150, 160, 150)
            } else {
                ramp(122, 150, 104)
            },
            hair: (!ghoul).then_some(BLACK_HAIR),
            face: Face::Zombie,
            kind: Body::Rags,
            body: ramp(110, 96, 90),
            right: if ghoul { W::Claws } else { W::None },
            eye: c(250, 240, 120),
            ..d
        });
    }
    if has(&["mummy"]) {
        return humanoid(&Hum {
            skin: ramp(214, 200, 160),
            hair: None,
            face: Face::Glow,
            eye: c(120, 230, 160),
            kind: Body::Cloth,
            body: ramp(214, 200, 160),
            legs: ramp(200, 186, 150),
            trim: c(150, 130, 90),
            head: if has(&["lord"]) {
                Head::Crown
            } else {
                Head::None
            },
            ..d
        });
    }
    if has(&["lich"]) {
        return humanoid(&Hum {
            skin: BONE,
            hair: None,
            face: Face::Skull,
            eye: c(90, 230, 255),
            head: Head::Crown,
            kind: Body::Robe,
            body: ramp(70, 40, 100),
            trim: GOLD.m,
            right: W::Staff(c(90, 230, 255)),
            ..d
        });
    }
    if has(&["vampire"]) {
        return humanoid(&Hum {
            skin: ramp(226, 220, 230),
            hair: Some(BLACK_HAIR),
            eye: c(230, 40, 50),
            kind: Body::Cloth,
            body: ramp(50, 40, 56),
            cape: Some(ramp(150, 30, 44)),
            trim: c(230, 40, 50),
            ..d
        });
    }
    if has(&["wight", "death knight", "revenant"]) {
        return humanoid(&Hum {
            skin: ramp(170, 190, 200),
            hair: Some(ramp(220, 220, 230)),
            long_hair: true,
            face: Face::Glow,
            eye: c(120, 220, 255),
            kind: Body::Plate,
            body: ramp(70, 70, 86),
            right: W::Great,
            ..d
        });
    }
    if has(&["wisp"]) {
        let mut p = Pix::new(32, 32);
        p.sell(16.0, 15.0, 7.0, 7.0, ramp(150, 230, 255));
        p.sell(16.0, 15.0, 3.5, 3.5, ramp(240, 252, 255));
        for (x, y) in [(6, 8), (26, 10), (9, 24), (24, 23)] {
            p.set(x, y, c(190, 240, 255));
        }
        p.ground_shadow(16.0, 30.0, 5.0, 1.2);
        return p;
    }
    if has(&[
        "ghost",
        "specter",
        "wraith",
        "shadow",
        "banshee",
        "poltergeist",
    ]) {
        return if has(&["shadow", "wraith"]) {
            ghost(ramp(60, 54, 80), c(230, 60, 60), 235)
        } else {
            ghost(ramp(190, 220, 230), c(120, 240, 255), 215)
        };
    }
    // ---- goblinoids, orcs and other folk
    if has(&["hobgoblin"]) {
        return humanoid(&Hum {
            skin: ramp(196, 110, 70),
            hair: Some(BLACK_HAIR),
            ears: true,
            face: Face::Tusks,
            head: Head::Helm {
                full: false,
                plume: None,
            },
            kind: Body::Plate,
            body: ramp(120, 50, 50),
            right: W::Sword,
            left: L::Shield(ramp(120, 50, 50), GOLD.m),
            ..d
        });
    }
    if has(&["bugbear"]) {
        return humanoid(&Hum {
            skin: ramp(150, 110, 70),
            hair: Some(ramp(110, 76, 46)),
            long_hair: true,
            ears: true,
            face: Face::Tusks,
            kind: Body::Cloth,
            body: ramp(100, 80, 60),
            right: W::Club,
            ..d
        });
    }
    if has(&["goblin"]) {
        let boss = has(&["boss", "king"]);
        return humanoid(&Hum {
            skin: goblin,
            hair: None,
            ears: true,
            short: true,
            eye: c(220, 40, 40),
            head: if boss {
                Head::Horned
            } else if h.is_multiple_of(3) {
                Head::Hood
            } else {
                Head::None
            },
            hat: if boss { IRON } else { ramp(120, 60, 50) },
            body: ramp(130, 90, 60),
            right: pick(&[W::Axe, W::Club, W::Dagger, W::Spear]),
            left: if h.is_multiple_of(2) {
                L::Shield(WOOD, c(190, 50, 50))
            } else {
                L::None
            },
            ..d
        });
    }
    if has(&["kobold"]) {
        return humanoid(&Hum {
            skin: ramp(190, 100, 60),
            hair: None,
            face: Face::Snout,
            horns: Some(BONE),
            short: true,
            eye: c(255, 220, 80),
            body: ramp(110, 90, 70),
            right: pick(&[W::Spear, W::Dagger]),
            ..d
        });
    }
    if has(&["orc", "orog"]) && !has(&["sorcer"]) {
        return humanoid(&Hum {
            skin: ramp(110, 136, 92),
            hair: Some(BLACK_HAIR),
            face: Face::Tusks,
            ears: true,
            head: if h.is_multiple_of(2) {
                Head::Horned
            } else {
                Head::None
            },
            hat: IRON,
            kind: Body::Bare,
            legs: ramp(110, 76, 50),
            right: pick(&[W::GreatAxe, W::Axe, W::Club]),
            eye: c(230, 60, 40),
            ..d
        });
    }
    if has(&["gnoll"]) {
        return humanoid(&Hum {
            skin: ramp(176, 146, 96),
            hair: Some(ramp(110, 80, 50)),
            face: Face::Snout,
            ears: true,
            kind: Body::Cloth,
            body: ramp(110, 80, 60),
            right: pick(&[W::Spear, W::Axe]),
            eye: c(230, 60, 40),
            ..d
        });
    }
    if has(&[
        "lizardfolk",
        "troglodyte",
        "sahuagin",
        "merfolk",
        "kuo-toa",
        "merrow",
        "bullywug",
    ]) {
        let sea = has(&["sahuagin", "merfolk", "kuo", "merrow"]);
        return humanoid(&Hum {
            skin: if sea {
                ramp(70, 150, 170)
            } else {
                ramp(90, 150, 100)
            },
            hair: None,
            face: Face::Snout,
            kind: Body::Bare,
            legs: ramp(100, 80, 60),
            right: W::Spear,
            left: if h.is_multiple_of(2) {
                L::Shield(WOOD, c(90, 150, 100))
            } else {
                L::None
            },
            eye: c(255, 220, 80),
            ..d
        });
    }
    if has(&["minotaur"]) {
        return humanoid(&Hum {
            skin: ramp(130, 90, 60),
            hair: None,
            face: Face::Snout,
            horns: Some(BONE),
            kind: Body::Bare,
            legs: ramp(90, 60, 44),
            right: W::GreatAxe,
            eye: c(230, 60, 40),
            ..d
        });
    }
    if has(&["ogre", "troll", "ettin", "cyclops", "giant"]) && ty.contains("giant")
        || has(&["ogre", "troll"])
    {
        let skin = if has(&["troll"]) {
            ramp(96, 140, 96)
        } else if has(&["frost"]) {
            ramp(170, 200, 230)
        } else if has(&["fire"]) {
            ramp(70, 60, 70)
        } else if has(&["stone"]) {
            STONE
        } else if has(&["cloud", "storm"]) {
            ramp(190, 200, 226)
        } else {
            ramp(196, 160, 110)
        };
        return humanoid(&Hum {
            skin,
            hair: has(&["fire"])
                .then_some(RED_HAIR)
                .or(has(&["frost", "storm"]).then_some(ramp(236, 240, 250))),
            beard: has(&["fire", "frost", "storm"]).then_some(if has(&["fire"]) {
                RED_HAIR
            } else {
                ramp(236, 240, 250)
            }),
            face: if has(&["troll", "ogre"]) {
                Face::Tusks
            } else {
                Face::Normal
            },
            ears: has(&["troll", "ogre"]),
            kind: Body::Bare,
            legs: ramp(110, 84, 60),
            right: if has(&["troll"]) {
                W::Claws
            } else {
                pick(&[W::Club, W::GreatAxe, W::Great])
            },
            ..d
        });
    }
    if has(&["hag", "witch"]) {
        return humanoid(&Hum {
            skin: ramp(120, 160, 110),
            hair: Some(ramp(230, 230, 220)),
            long_hair: true,
            head: Head::WizHat,
            hat: ramp(50, 44, 60),
            kind: Body::Robe,
            body: ramp(60, 50, 70),
            trim: c(120, 160, 110),
            right: W::Staff(c(140, 230, 90)),
            eye: c(255, 220, 80),
            ..d
        });
    }
    // ---- fiends and celestials
    if has(&["imp", "quasit", "mephit", "homunculus", "sprite", "pixie"]) {
        let skin = if has(&["sprite", "pixie"]) {
            FAIR
        } else if has(&["ice"]) {
            ramp(170, 210, 240)
        } else if has(&["dust", "mud"]) {
            ramp(150, 120, 90)
        } else if has(&["steam", "smoke"]) {
            ramp(170, 170, 180)
        } else if has(&["quasit"]) {
            ramp(90, 150, 90)
        } else {
            demon
        };
        let fey = has(&["sprite", "pixie"]);
        return humanoid(&Hum {
            skin,
            hair: fey.then_some(BLOND),
            short: true,
            ears: true,
            horns: (!fey).then_some(BONE),
            wings: Some((
                if fey {
                    ramp(190, 230, 250)
                } else {
                    ramp(skin.d[0], skin.d[1], skin.d[2])
                },
                fey,
            )),
            kind: if fey { Body::Cloth } else { Body::Bare },
            body: LEAF,
            legs: ramp(80, 40, 40),
            right: if fey { W::Dagger } else { W::Claws },
            eye: c(255, 220, 80),
            ..d
        });
    }
    if has(&["succubus", "incubus", "erinyes"]) {
        return humanoid(&Hum {
            skin: ramp(226, 180, 190),
            hair: Some(BLACK_HAIR),
            long_hair: true,
            horns: Some(ramp(70, 40, 50)),
            wings: Some((ramp(120, 30, 50), false)),
            kind: Body::Cloth,
            body: ramp(110, 30, 60),
            right: if has(&["erinyes"]) { W::Sword } else { W::None },
            eye: c(240, 60, 120),
            ..d
        });
    }
    if has(&["gargoyle"]) {
        return humanoid(&Hum {
            skin: STONE,
            hair: None,
            horns: Some(STONE),
            ears: true,
            wings: Some((ramp(96, 94, 96), false)),
            kind: Body::Bare,
            legs: ramp(96, 94, 96),
            right: W::Claws,
            face: Face::Glow,
            eye: c(255, 200, 60),
            ..d
        });
    }
    if ty.contains("fiend") && !has(&["hound", "nightmare", "rakshasa"]) {
        let big = has(&[
            "balor",
            "pit fiend",
            "horned",
            "marilith",
            "nalfeshnee",
            "glabrezu",
        ]);
        let skin = if has(&["ice", "bone"]) {
            ramp(190, 210, 226)
        } else if has(&[
            "barbed", "bearded", "chain", "vrock", "hezrou", "dretch", "lemure", "manes",
        ]) {
            ramp(120, 96, 110)
        } else {
            demon
        };
        return humanoid(&Hum {
            skin,
            hair: None,
            horns: Some(ramp(60, 40, 50)),
            ears: true,
            face: Face::Tusks,
            wings: big.then_some((ramp(110, 26, 36), false)),
            kind: Body::Bare,
            legs: ramp(70, 40, 44),
            right: if big {
                pick(&[W::Great, W::GreatAxe])
            } else {
                pick(&[W::Spear, W::Claws, W::Scythe])
            },
            eye: c(255, 210, 60),
            ..d
        });
    }
    if ty.contains("celestial") && !has(&["couatl", "unicorn", "pegasus"]) {
        return humanoid(&Hum {
            skin: ramp(250, 226, 190),
            hair: Some(BLOND),
            long_hair: true,
            wings: Some((ramp(244, 244, 250), true)),
            kind: Body::Robe,
            body: ramp(244, 240, 226),
            trim: GOLD.l,
            right: pick(&[W::Great, W::Mace, W::Spear]),
            eye: c(90, 180, 250),
            head: Head::Crown,
            ..d
        });
    }
    // ---- oozes, constructs, elementals, plants
    if has(&["cube"]) {
        return slime(ramp(140, 220, 200), true);
    }
    if has(&["ooze", "jelly", "pudding", "slime"]) || ty.contains("ooze") {
        return slime(
            if has(&["black"]) {
                ramp(56, 50, 66)
            } else if has(&["ochre"]) {
                ramp(210, 170, 60)
            } else if has(&["gray", "grey"]) {
                ramp(150, 150, 156)
            } else {
                ramp(100, 190, 90)
            },
            false,
        );
    }
    if has(&["mimic"]) {
        return mimic();
    }
    if has(&["fire elemental", "magmin", "fire snake"])
        || ty.contains("elemental") && has(&["fire", "magma"])
    {
        return flame_elemental();
    }
    if has(&["water", "ice", "steam"]) && ty.contains("elemental") {
        return blob_elemental(ramp(70, 150, 220), false);
    }
    if has(&["air", "dust", "smoke", "invisible stalker", "djinni"]) && ty.contains("elemental") {
        return blob_elemental(ramp(200, 214, 226), true);
    }
    if has(&["earth elemental", "xorn", "galeb"]) {
        return rock_elemental(ramp(140, 110, 80), c(255, 210, 80));
    }
    if has(&["golem", "guardian", "colossus"]) {
        let r = if has(&["iron"]) {
            IRON
        } else if has(&["clay"]) {
            ramp(176, 120, 84)
        } else if has(&["flesh"]) {
            ramp(160, 150, 130)
        } else {
            STONE
        };
        return rock_elemental(r, c(120, 230, 255));
    }
    if has(&["animated armor", "helmed horror"]) {
        return humanoid(&Hum {
            head: Head::Helm {
                full: true,
                plume: None,
            },
            kind: Body::Plate,
            body: STEEL,
            legs: IRON,
            right: W::Sword,
            eye: c(120, 220, 255),
            ..d
        });
    }
    if has(&["scarecrow"]) {
        return humanoid(&Hum {
            skin: ramp(210, 180, 110),
            hair: None,
            face: Face::Glow,
            eye: c(255, 150, 40),
            head: Head::Cap(None),
            hat: ramp(100, 70, 50),
            kind: Body::Rags,
            body: ramp(120, 80, 60),
            right: W::Scythe,
            ..d
        });
    }
    if has(&["myconid", "fungus", "shrieker", "mold"]) {
        return plant(
            if has(&["violet"]) {
                ramp(150, 90, 190)
            } else {
                ramp(190, 80, 70)
            },
            true,
        );
    }
    if ty.contains("plant") || has(&["treant", "shrub", "blight", "shambling"]) {
        return plant(
            if has(&["blight"]) {
                ramp(110, 100, 60)
            } else {
                LEAF
            },
            false,
        );
    }
    // ---- beasts
    if has(&["spider", "ettercap", "drider"]) {
        return spider(
            if has(&["phase"]) {
                ramp(90, 120, 200)
            } else {
                ramp(70, 60, 76)
            },
            false,
        );
    }
    if has(&[
        "scorpion",
        "crab",
        "ankheg",
        "beetle",
        "centipede",
        "remorhaz",
        "carrion",
    ]) {
        return spider(
            if has(&["crab"]) {
                ramp(210, 110, 80)
            } else {
                ramp(150, 110, 60)
            },
            true,
        );
    }
    if has(&["bat", "stirge", "wasp", "cloaker", "darkmantle"]) {
        return flyer(ramp(96, 76, 90), false);
    }
    if has(&[
        "eagle",
        "hawk",
        "owl",
        "raven",
        "vulture",
        "roc",
        "bird",
        "harpy",
        "cockatrice",
        "axe beak",
        "pteranodon",
    ]) && !has(&["owlbear"])
    {
        return flyer(
            if has(&["raven", "vulture"]) {
                ramp(60, 56, 70)
            } else if has(&["owl"]) {
                ramp(200, 190, 170)
            } else {
                ramp(150, 110, 70)
            },
            true,
        );
    }
    if has(&[
        "snake",
        "constrictor",
        "viper",
        "worm",
        "naga",
        "couatl",
        "eel",
        "behir",
        "salamander",
    ]) {
        return if has(&["purple"]) {
            snake(ramp(130, 70, 160), c(230, 190, 200))
        } else if has(&["salamander"]) {
            snake(ramp(220, 90, 40), c(255, 210, 100))
        } else if has(&["couatl"]) {
            snake(ramp(90, 190, 170), c(250, 230, 150))
        } else {
            snake(ramp(96, 150, 70), c(226, 220, 150))
        };
    }
    if has(&[
        "shark",
        "fish",
        "quipper",
        "whale",
        "octopus",
        "squid",
        "kraken",
        "aboleth",
        "dolphin",
        "sea horse",
    ]) {
        return if has(&["octopus", "squid", "kraken", "aboleth"]) {
            eye_beast(ramp(110, 90, 150))
        } else {
            fish(ramp(110, 140, 170), has(&["shark", "quipper", "whale"]))
        };
    }
    if has(&["hell hound", "nightmare"]) {
        return quadruped(&Quad {
            eye: c(255, 120, 30),
            mane: has(&["nightmare"]).then_some(ramp(250, 130, 40)),
            stripes: Some(c(250, 130, 40)),
            ..Quad::new(ramp(60, 44, 50))
        });
    }
    if has(&[
        "wolf", "worg", "dog", "mastiff", "jackal", "hyena", "fox", "blink",
    ]) {
        let r = if has(&["winter"]) {
            ramp(226, 232, 240)
        } else if has(&["dire", "worg"]) {
            ramp(84, 80, 96)
        } else if has(&["jackal", "hyena", "mastiff"]) {
            ramp(176, 140, 90)
        } else {
            ramp(128, 132, 146)
        };
        return quadruped(&Quad {
            eye: if has(&["dire", "worg", "winter"]) {
                c(230, 50, 50)
            } else {
                c(250, 220, 80)
            },
            ..Quad::new(r)
        });
    }
    if has(&["bear", "owlbear"]) {
        return quadruped(&Quad {
            bulk: 1.3,
            pointed_ears: false,
            bushy_tail: false,
            long_snout: !has(&["owlbear"]),
            ..Quad::new(if has(&["polar"]) {
                ramp(236, 236, 230)
            } else if has(&["black"]) {
                ramp(66, 60, 66)
            } else {
                ramp(130, 90, 60)
            })
        });
    }
    if has(&[
        "lion",
        "tiger",
        "panther",
        "cat",
        "leopard",
        "saber",
        "displacer",
        "manticore",
        "chimera",
        "sphinx",
        "griffon",
        "rakshasa",
        "lamia",
    ]) {
        let tiger = has(&["tiger", "saber", "rakshasa"]);
        return quadruped(&Quad {
            bushy_tail: false,
            long_snout: false,
            mane: has(&["lion", "manticore", "chimera", "sphinx"]).then_some(ramp(150, 90, 40)),
            stripes: tiger.then_some(c(60, 40, 40)),
            tusks: has(&["saber"]),
            wings: has(&["manticore", "chimera", "sphinx", "griffon"]).then_some(
                if has(&["griffon", "sphinx"]) {
                    ramp(230, 220, 200)
                } else {
                    ramp(120, 60, 60)
                },
            ),
            ..Quad::new(if has(&["panther", "displacer"]) {
                ramp(60, 56, 80)
            } else if tiger {
                ramp(226, 140, 60)
            } else {
                ramp(214, 170, 96)
            })
        });
    }
    if has(&[
        "boar",
        "elephant",
        "mammoth",
        "rhinoceros",
        "hippo",
        "triceratops",
        "gorgon",
        "bull",
        "ox",
        "cow",
        "bulette",
        "ankylosaurus",
    ]) {
        return quadruped(&Quad {
            bulk: 1.25,
            bushy_tail: false,
            tusks: true,
            pointed_ears: false,
            horns: has(&["gorgon", "bull", "ox", "triceratops", "rhino"]),
            ..Quad::new(if has(&["gorgon"]) {
                IRON
            } else if has(&["mammoth", "boar"]) {
                ramp(120, 84, 60)
            } else {
                ramp(140, 136, 140)
            })
        });
    }
    if has(&[
        "horse",
        "pony",
        "mule",
        "camel",
        "elk",
        "deer",
        "goat",
        "unicorn",
        "pegasus",
        "hippogriff",
        "centaur",
    ]) {
        let white = has(&["unicorn", "pegasus"]);
        return quadruped(&Quad {
            bushy_tail: true,
            pointed_ears: true,
            horns: has(&["elk", "deer", "goat", "unicorn"]),
            wings: has(&["pegasus", "hippogriff"]).then_some(ramp(240, 240, 246)),
            mane: Some(if white {
                ramp(250, 230, 160)
            } else {
                ramp(70, 50, 40)
            }),
            eye: EYE,
            ..Quad::new(if white {
                ramp(240, 240, 246)
            } else {
                fur(h + 1)
            })
        });
    }
    if has(&[
        "crocodile",
        "lizard",
        "basilisk",
        "frog",
        "toad",
        "tyrannosaurus",
        "allosaurus",
        "plesiosaurus",
        "hydra",
    ]) {
        return quadruped(&Quad {
            bushy_tail: false,
            pointed_ears: false,
            bulk: 0.9,
            ..Quad::new(if has(&["frog", "toad"]) {
                ramp(110, 170, 80)
            } else {
                ramp(84, 130, 84)
            })
        });
    }
    if has(&["rat", "weasel", "badger", "ape", "baboon"]) {
        return quadruped(&Quad {
            bushy_tail: false,
            bulk: 0.85,
            eye: c(230, 50, 50),
            ..Quad::new(ramp(120, 104, 100))
        });
    }
    // ---- named humanoid roles
    let guard = Hum {
        head: Head::Helm {
            full: false,
            plume: None,
        },
        kind: Body::Plate,
        body: STEEL,
        legs: IRON,
        right: W::Spear,
        left: L::Shield(ramp(70, 100, 170), GOLD.l),
        ..d
    };
    if has(&["knight", "champion", "warlord", "captain"]) {
        return humanoid(&Hum {
            head: Head::Helm {
                full: true,
                plume: Some(c(60, 90, 200)),
            },
            right: W::Sword,
            cape: Some(ramp(60, 90, 170)),
            ..guard
        });
    }
    if has(&["guard", "soldier", "veteran", "warrior", "gladiator"]) {
        return humanoid(&guard);
    }
    if has(&[
        "mage",
        "wizard",
        "sorcer",
        "archmage",
        "illusionist",
        "necromancer",
        "warlock",
    ]) {
        let col = if has(&["necro", "warlock"]) {
            ramp(60, 44, 80)
        } else if has(&["arch"]) {
            ramp(140, 50, 150)
        } else {
            ramp(150, 50, 60)
        };
        return humanoid(&Hum {
            head: Head::WizHat,
            hat: col,
            kind: Body::Robe,
            body: col,
            beard: has(&["arch"]).then_some(ramp(230, 230, 236)),
            hair: Some(BLACK_HAIR),
            right: W::Staff(c(250, 120, 220)),
            ..d
        });
    }
    if has(&["cultist", "fanatic", "acolyte", "priest", "druid"]) {
        let dark = has(&["cult", "fanatic"]);
        return humanoid(&Hum {
            head: if dark { Head::Hood } else { Head::None },
            hat: ramp(110, 30, 40),
            kind: Body::Robe,
            body: if dark {
                ramp(110, 30, 40)
            } else if has(&["druid"]) {
                LEAF
            } else {
                ramp(220, 214, 200)
            },
            trim: GOLD.m,
            right: if dark { W::Dagger } else { W::Mace },
            face: if dark { Face::Glow } else { Face::Normal },
            eye: if dark { c(255, 200, 80) } else { EYE },
            hair: Some(BROWN_HAIR),
            ..d
        });
    }
    if has(&[
        "bandit",
        "thug",
        "pirate",
        "spy",
        "assassin",
        "scout",
        "thief",
        "berserker",
        "tough",
    ]) {
        let berserk = has(&["berserker"]);
        return humanoid(&Hum {
            head: if berserk {
                Head::None
            } else if has(&["assassin", "spy", "scout"]) {
                Head::Hood
            } else {
                Head::Bandana
            },
            hat: if has(&["assassin"]) {
                ramp(50, 46, 60)
            } else {
                ramp(170, 50, 50)
            },
            kind: if berserk { Body::Bare } else { Body::Cloth },
            body: ramp(110, 84, 60),
            hair: Some(if berserk { RED_HAIR } else { BLACK_HAIR }),
            beard: berserk.then_some(RED_HAIR),
            skin: [FAIR, TAN, DARK_SKIN][(h % 3) as usize],
            right: if berserk {
                W::GreatAxe
            } else {
                pick(&[W::Sword, W::Dagger, W::Club, W::Bow])
            },
            ..d
        });
    }
    if has(&["noble", "commoner", "king", "queen", "merchant"]) {
        return humanoid(&Hum {
            head: if has(&["noble", "king", "queen"]) {
                Head::Crown
            } else {
                Head::None
            },
            body: if has(&["commoner"]) {
                ramp(150, 130, 100)
            } else {
                ramp(120, 50, 130)
            },
            hair: Some(BLOND),
            ..d
        });
    }
    // ---- fall back on the creature type
    match ty.split_whitespace().next().unwrap_or("") {
        "beast" => quadruped(&Quad {
            bushy_tail: h.is_multiple_of(2),
            ..Quad::new(fur(h))
        }),
        "monstrosity" => quadruped(&Quad {
            horns: true,
            bushy_tail: false,
            eye: c(230, 50, 50),
            ..Quad::new(
                [ramp(130, 80, 110), ramp(90, 110, 130), ramp(150, 110, 70)][(h % 3) as usize],
            )
        }),
        "dragon" => {
            let (r, b) = dragon_colors(n, h);
            dragon(r, b)
        }
        "undead" => humanoid(&Hum {
            skin: BONE,
            hair: None,
            face: Face::Skull,
            kind: Body::Bones,
            eye: c(230, 60, 50),
            right: W::Sword,
            ..d
        }),
        "aberration" => eye_beast(
            [ramp(130, 90, 150), ramp(90, 140, 130), ramp(170, 100, 100)][(h % 3) as usize],
        ),
        "construct" => rock_elemental(STONE, c(120, 230, 255)),
        "elemental" => rock_elemental(ramp(140, 110, 80), c(255, 210, 80)),
        "fey" => humanoid(&Hum {
            skin: ramp(190, 226, 180),
            hair: Some(ramp(230, 130, 170)),
            long_hair: true,
            ears: true,
            kind: Body::Robe,
            body: LEAF,
            trim: c(240, 210, 110),
            right: W::Staff(c(250, 150, 220)),
            ..d
        }),
        "giant" => humanoid(&Hum {
            skin: ramp(196, 160, 110),
            kind: Body::Bare,
            legs: ramp(110, 84, 60),
            right: W::Club,
            beard: Some(BROWN_HAIR),
            ..d
        }),
        _ => humanoid(&Hum {
            skin: [FAIR, TAN, DARK_SKIN][(h % 3) as usize],
            hair: Some([BROWN_HAIR, BLACK_HAIR, BLOND][(h / 3 % 3) as usize]),
            body: [LEATHER, ramp(90, 110, 140), ramp(120, 60, 60)][(h / 9 % 3) as usize],
            right: pick(&[W::Sword, W::Spear, W::Axe, W::Bow]),
            ..d
        }),
    }
}

/// Sprite for a key: `class:<token icon>` or `mon:<creature type>:<name>` (lower-case).
pub fn creature(key: &str) -> Pix {
    let mut parts = key.splitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("class"), Some(icon), _) => humanoid(&class_sprite(icon)),
        (Some("mon"), Some(ty), Some(name)) => monster(ty, name),
        _ => monster("", key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const SAMPLE: [&str; 40] = [
        "mon:humanoid:goblin warrior",
        "mon:humanoid:hobgoblin",
        "mon:humanoid:bugbear",
        "mon:humanoid:orc",
        "mon:humanoid:kobold",
        "mon:undead:skeleton",
        "mon:undead:zombie",
        "mon:undead:ghoul",
        "mon:undead:lich",
        "mon:undead:ghost",
        "mon:undead:vampire",
        "mon:undead:mummy",
        "mon:ooze:ochre jelly",
        "mon:ooze:gelatinous cube",
        "mon:beast:wolf",
        "mon:beast:dire wolf",
        "mon:beast:brown bear",
        "mon:beast:giant spider",
        "mon:beast:giant bat",
        "mon:beast:giant eagle",
        "mon:beast:giant constrictor snake",
        "mon:beast:tiger",
        "mon:beast:boar",
        "mon:elemental:fire elemental",
        "mon:elemental:water elemental",
        "mon:elemental:air elemental",
        "mon:elemental:earth elemental",
        "mon:fiend:imp",
        "mon:fiend:balor",
        "mon:fiend:hell hound",
        "mon:construct:stone golem",
        "mon:construct:animated armor",
        "mon:dragon:adult red dragon",
        "mon:dragon:young blue dragon",
        "mon:aberration:aboleth",
        "mon:plant:treant",
        "mon:plant:violet fungus",
        "mon:monstrosity:mimic",
        "mon:giant:ogre",
        "mon:celestial:deva",
    ];

    #[test]
    fn classes_and_common_monsters_have_distinct_sprites() {
        let mut keys: Vec<String> = dnd_rules::character::TOKEN_ICONS
            .iter()
            .map(|i| format!("class:{i}"))
            .collect();
        keys.extend(SAMPLE.iter().map(|s| s.to_string()));
        let imgs: Vec<Pix> = keys.iter().map(|k| creature(k)).collect();
        for (k, p) in keys.iter().zip(&imgs) {
            assert!(
                p.px.iter().filter(|c| c[3] > 100).count() > 150,
                "{k} is nearly empty"
            );
        }
        for i in 0..imgs.len() {
            for j in i + 1..imgs.len() {
                let same = imgs[i].w == imgs[j].w
                    && imgs[i]
                        .px
                        .iter()
                        .zip(&imgs[j].px)
                        .filter(|(a, b)| a != b)
                        .count()
                        < 40;
                assert!(!same, "{} and {} share a sprite", keys[i], keys[j]);
            }
        }
    }

    /// `DND55_SPRITE_SHEET=/path.rgba cargo test dump_sprite_sheet -- --ignored`
    #[test]
    #[ignore]
    fn dump_sprite_sheet() {
        let Ok(path) = std::env::var("DND55_SPRITE_SHEET") else {
            return;
        };
        let mut keys: Vec<String> = dnd_rules::character::TOKEN_ICONS
            .iter()
            .map(|i| format!("class:{i}"))
            .collect();
        keys.extend(SAMPLE.iter().map(|s| s.to_string()));
        let (cols, cell) = (9usize, 48usize);
        let rows = keys.len().div_ceil(cols);
        let (w, h) = (cols * cell, rows * cell);
        let mut out = vec![110u8; w * h * 4];
        for (k, key) in keys.iter().enumerate() {
            let p = creature(key);
            let (ox, oy) = (
                k % cols * cell + (cell - p.w as usize) / 2,
                k / cols * cell + (cell - p.h as usize) / 2,
            );
            for y in 0..p.h as usize {
                for x in 0..p.w as usize {
                    let c = p.get(x as i32, y as i32);
                    let i = ((oy + y) * w + ox + x) * 4;
                    let a = c[3] as f32 / 255.0;
                    for ch in 0..3 {
                        out[i + ch] = (c[ch] as f32 * a + 110.0 * (1.0 - a)) as u8;
                    }
                }
            }
        }
        for px in out.chunks_mut(4) {
            px[3] = 255;
        }
        std::fs::write(&path, out).unwrap();
        std::fs::write(format!("{path}.size"), format!("{w} {h}")).unwrap();
    }
}
