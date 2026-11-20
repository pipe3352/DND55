//! Merchants (auto and DM-approved), campaign time and rests, timed conditions, prefabs, templates.

use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_session::command::{Command, DmCommand};
use dnd_session::model::{MapTemplate, Merchant, Principal, ShopItem};
use dnd_session::{CommandError, Session};
use dnd_world::map::LayerKind;
use dnd_world::{EntityId, RoomId};

const DM: Principal = Principal::Dm;

fn dm(s: &mut Session, c: DmCommand) {
    s.apply(DM, Command::Dm(c)).expect("dm command");
}

fn setup() -> (Session, RoomId, Principal, EntityId) {
    let mut s = Session::new_campaign("Shops");
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::FillRect {
            room,
            layer: LayerKind::Terrain,
            a: (0, 0),
            b: (19, 14),
            asset: Some("grass".into()),
        },
    );
    let p = s.add_player("Alice");
    let who = Principal::Player(p);
    s.apply(
        who,
        Command::SubmitCharacter {
            sheet: Box::new(CharacterSheet::example_fighter("Alice")),
            portrait_png: None,
        },
    )
    .unwrap();
    let alice = s.state.players[&p].character.unwrap();
    let c = &mut s
        .state
        .creatures
        .get_mut(&alice)
        .unwrap()
        .sheet_mut()
        .unwrap()
        .currency;
    *c = Default::default();
    c.gp = 10;
    (s, room, who, alice)
}

fn shop(auto: bool) -> Merchant {
    let mut potion = dnd_rules::items::catalog_item("potion_of_healing").expect("catalog potion");
    potion.quantity = 1;
    Merchant {
        name: "Mira".into(),
        open: true,
        auto_approve: auto,
        stock: vec![ShopItem {
            item: potion,
            price_cp: 250,
            stock: Some(2),
        }],
        ..Default::default()
    }
}

#[test]
fn buying_selling_and_dm_approval() {
    let (mut s, _, who, alice) = setup();
    dm(
        &mut s,
        DmCommand::SaveMerchant {
            merchant: shop(true),
        },
    );
    let mid = s.state.merchants[0].id;
    s.apply(
        who,
        Command::Buy {
            entity: alice,
            merchant: mid,
            item: 0,
            quantity: 2,
        },
    )
    .unwrap();
    let sh = s.state.creatures[&alice].sheet().unwrap().clone();
    assert_eq!(sh.currency.total_cp(), 1000 - 500);
    assert!(
        sh.inventory
            .iter()
            .find(|i| i.id == "potion_of_healing")
            .unwrap()
            .quantity
            >= 2
    );
    assert_eq!(s.state.merchants[0].stock[0].stock, Some(0));
    // Out of stock / not enough money are refused.
    assert!(matches!(
        s.apply(
            who,
            Command::Buy {
                entity: alice,
                merchant: mid,
                item: 0,
                quantity: 1
            }
        ),
        Err(CommandError::Rules(_))
    ));
    // Sell one back at the buy rate (50% of its value).
    let value = sh
        .inventory
        .iter()
        .find(|i| i.id == "potion_of_healing")
        .unwrap()
        .value_cp as u64;
    s.apply(
        who,
        Command::Sell {
            entity: alice,
            merchant: mid,
            item_id: "potion_of_healing".into(),
            quantity: 1,
        },
    )
    .unwrap();
    assert_eq!(
        s.state.creatures[&alice]
            .sheet()
            .unwrap()
            .currency
            .total_cp(),
        500 + value / 2
    );
    // DM approval: the request waits, the player sees it, the DM approves.
    let mut m = shop(false);
    m.name = "Grim".into();
    dm(&mut s, DmCommand::SaveMerchant { merchant: m });
    let m2 = s.state.merchants[1].id;
    let before = s.state.creatures[&alice]
        .sheet()
        .unwrap()
        .currency
        .total_cp();
    s.apply(
        who,
        Command::Buy {
            entity: alice,
            merchant: m2,
            item: 0,
            quantity: 1,
        },
    )
    .unwrap();
    assert_eq!(
        s.state.creatures[&alice]
            .sheet()
            .unwrap()
            .currency
            .total_cp(),
        before
    );
    assert_eq!(s.build_view(who, None).trades.len(), 1);
    let tid = s.state.trades[0].id;
    dm(
        &mut s,
        DmCommand::ResolveTrade {
            id: tid,
            approve: true,
        },
    );
    assert_eq!(
        s.state.creatures[&alice]
            .sheet()
            .unwrap()
            .currency
            .total_cp(),
        before - 250
    );
    // Closed shops are invisible and unusable for players.
    let mut closed = s.state.merchants[1].clone();
    closed.open = false;
    dm(&mut s, DmCommand::SaveMerchant { merchant: closed });
    assert_eq!(s.build_view(who, None).merchants.len(), 1);
    assert!(
        s.apply(
            who,
            Command::Buy {
                entity: alice,
                merchant: m2,
                item: 0,
                quantity: 1
            }
        )
        .is_err()
    );
    assert!(matches!(
        s.apply(
            who,
            Command::Dm(DmCommand::SaveMerchant {
                merchant: shop(true)
            })
        ),
        Err(CommandError::NotAuthorized)
    ));
}

#[test]
fn time_rests_and_timed_conditions() {
    let (mut s, room, who, alice) = setup();
    assert_eq!(s.state.time.label(), "Day 1, 08:00");
    dm(&mut s, DmCommand::AdvanceTime { minutes: 17 * 60 });
    assert_eq!(s.state.time.label(), "Day 2, 01:00");
    dm(&mut s, DmCommand::Rest { long: true });
    assert_eq!(s.state.time.label(), "Day 2, 09:00");
    dm(&mut s, DmCommand::Rest { long: false });
    assert_eq!(s.state.time.minute, 10 * 60);
    assert_eq!(s.build_view(who, None).time, s.state.time);
    // Timed condition: 2 rounds, removed at the end of the second turn.
    dm(
        &mut s,
        DmCommand::SpawnMonster {
            template: "goblin_warrior".into(),
            room,
            cell: (8, 8),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::SetCondition {
            entity: alice,
            condition: Condition::Poisoned,
            present: true,
            rounds: Some(2),
        },
    );
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    for _ in 0..4 {
        dm(&mut s, DmCommand::NextTurn);
    }
    assert!(
        !s.state.creatures[&alice]
            .conditions
            .has(Condition::Poisoned)
    );
    assert!(s.state.creatures[&alice].condition_rounds.is_empty());
}

#[test]
fn prefabs_and_templates() {
    let (mut s, room, _, _) = setup();
    dm(
        &mut s,
        DmCommand::ApplyTemplate {
            room,
            template: MapTemplate::Tavern,
        },
    );
    let rs = &s.state.rooms[&room];
    assert!(rs.map.objects.iter().any(|o| o.asset_id == "bar_counter"));
    assert!(rs.map.tile(LayerKind::Walls, (0, 0)).is_some());
    let ids: Vec<_> = rs.map.objects.iter().map(|o| o.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "unique object ids");
    // Save two objects as a prefab, place it elsewhere.
    let pick: Vec<_> = rs
        .map
        .objects
        .iter()
        .filter(|o| o.asset_id == "barrel")
        .map(|o| o.id)
        .collect();
    dm(
        &mut s,
        DmCommand::SavePrefab {
            room,
            name: "Barrels".into(),
            objects: pick.clone(),
        },
    );
    let pid = s.state.prefabs[0].id;
    let n = s.state.rooms[&room].map.objects.len();
    dm(
        &mut s,
        DmCommand::PlacePrefab {
            room,
            prefab: pid,
            cell: (2, 10),
        },
    );
    assert_eq!(s.state.rooms[&room].map.objects.len(), n + pick.len());
    // Undo removes the placed prefab; every template works on this map.
    dm(&mut s, DmCommand::Undo { room });
    assert_eq!(s.state.rooms[&room].map.objects.len(), n);
    for t in MapTemplate::ALL {
        dm(&mut s, DmCommand::ApplyTemplate { room, template: t });
        assert!(!s.state.rooms[&room].map.objects.is_empty(), "{t:?}");
    }
    let json = serde_json::to_string(&s.state).unwrap();
    let back: dnd_session::GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s.state);
}
