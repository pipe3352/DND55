//! Commands are the only way to change the game state. Clients send them; the host validates them.

use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_rules::d20::RollMode;
use dnd_world::map::{AmbientLight, Cell, LayerKind};
use dnd_world::{EdgeId, EntityId, ObjectId, RoomId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Chat {
        text: String,
    },
    RollDice {
        expr: String,
        mode: RollMode,
        private: bool,
        label: String,
    },
    /// Create or replace the player's character. Portrait is a PNG (validated and re-encoded by the host).
    SubmitCharacter {
        sheet: Box<CharacterSheet>,
        #[serde(with = "crate::hexbytes::opt")]
        portrait_png: Option<Vec<u8>>,
    },
    SetReady(bool),
    Move {
        entity: EntityId,
        to: Cell,
    },
    Attack {
        attacker: EntityId,
        target: EntityId,
        attack: usize,
    },
    Dash {
        entity: EntityId,
    },
    Disengage {
        entity: EntityId,
    },
    Dodge {
        entity: EntityId,
    },
    EndTurn,
    /// Answer to a pending Opportunity Attack prompt.
    ReactionDecision {
        accept: bool,
        attack: usize,
    },
    Interact {
        entity: EntityId,
        object: ObjectId,
    },
    /// Cast a spell. `slot` is the spell slot level (None for cantrips). Targets/point depend on the spell.
    CastSpell {
        caster: EntityId,
        spell: String,
        slot: Option<u8>,
        targets: Vec<EntityId>,
        point: Option<Cell>,
    },
    /// Use a class feature (Rage, Second Wind, Flurry of Blows...).
    UseFeature {
        entity: EntityId,
        feature: Feature,
        target: Option<EntityId>,
        amount: Option<i32>,
    },
    /// Response of an attacked creature: Shield / Uncanny Dodge / Deflect Attacks, or None to take the hit.
    HitReaction {
        choice: Option<crate::model::HitReaction>,
    },
    EndConcentration {
        entity: EntityId,
    },
    /// Change prepared spells and known cantrips (outside combat).
    PrepareSpells {
        entity: EntityId,
        cantrips: Vec<String>,
        prepared: Vec<String>,
    },
    /// Advance one level (requires the XP threshold or a DM milestone).
    LevelUp {
        entity: EntityId,
        choices: Box<LevelUpChoices>,
    },
    /// Spend one Hit Point Die (outside combat, e.g. during a Short Rest).
    SpendHitDie {
        entity: EntityId,
    },
    UseItem {
        entity: EntityId,
        item: String,
        target: Option<EntityId>,
    },
    EquipItem {
        entity: EntityId,
        item: String,
        equip: bool,
    },
    GiveItem {
        from: EntityId,
        to: EntityId,
        item: String,
        quantity: u32,
    },
    TakeFromContainer {
        entity: EntityId,
        object: ObjectId,
        item: String,
        quantity: u32,
    },
    /// Use a monster's save-based action (breath weapon...) on a point/targets.
    MonsterAction {
        monster: EntityId,
        action: usize,
        targets: Vec<EntityId>,
        point: Option<Cell>,
    },
    DialogueChoice {
        choice: usize,
    },
    AddDrawing {
        room: RoomId,
        drawing: Box<crate::model::Drawing>,
    },
    Dm(DmCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feature {
    Rage,
    EndRage,
    RecklessAttack,
    SecondWind,
    ActionSurge,
    CunningDash,
    CunningDisengage,
    SteadyAim,
    MartialArtsStrike,
    FlurryOfBlows,
    PatientDefense { focus: bool },
    StepOfTheWind { focus: bool },
    StunningStrike,
    LayOnHands,
    BardicInspiration,
    UseInspiration,
    DivineSparkHeal,
    DivineSparkDamage,
    InnateSorcery,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelUpChoices {
    /// Ability Score Improvement feat at ASI levels: +2 to one or +1 to two.
    pub asi: Vec<(dnd_rules::Ability, u8)>,
    pub fighting_style: Option<dnd_rules::character::FightingStyle>,
    pub expertise: Vec<dnd_rules::Skill>,
    pub new_cantrips: Vec<String>,
    pub new_prepared: Vec<String>,
    pub new_masteries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DmCommand {
    /// Map editor: replace an object's properties (same id).
    UpdateObject {
        room: RoomId,
        object: Box<dnd_world::map::MapObject>,
    },
    /// Paste/duplicate objects (new ids are assigned).
    AddObjects {
        room: RoomId,
        objects: Vec<dnd_world::map::MapObject>,
    },
    RemoveObjects {
        room: RoomId,
        objects: Vec<ObjectId>,
    },
    MoveObjects {
        room: RoomId,
        objects: Vec<ObjectId>,
        dx: i32,
        dy: i32,
    },
    /// Move a layer up/down in the draw order.
    MoveLayer {
        room: RoomId,
        layer: LayerKind,
        up: bool,
    },
    SetRoomPos {
        room: RoomId,
        pos: (f32, f32),
    },
    DeleteRoom {
        room: RoomId,
    },
    SetRoomAudio {
        room: RoomId,
        music: Option<String>,
        ambience: Option<String>,
    },
    /// Edit every property of a connection (the graph editor's edge panel).
    EditEdge {
        edge: EdgeId,
        one_way: bool,
        locked: bool,
        secret: bool,
        label: String,
    },
    /// Room grouping and secrecy (graph editor).
    UpdateRoomMeta {
        room: RoomId,
        region: String,
        chapter: String,
        secret: bool,
    },
    /// Cosmetic weather/atmosphere for a room.
    SetRoomWeather {
        room: RoomId,
        weather: crate::model::Weather,
    },
    SaveCustomMonster {
        template: Box<dnd_rules::monster::MonsterTemplate>,
    },
    DeleteCustomMonster {
        id: String,
    },
    AwardXp {
        entities: Vec<EntityId>,
        xp: u32,
    },
    GrantLevel {
        entity: EntityId,
    },
    Rest {
        long: bool,
    },
    EndEffect {
        effect: u64,
    },
    SetContainer {
        room: RoomId,
        object: ObjectId,
        items: Vec<dnd_rules::items::ItemStack>,
    },
    GiveItemTo {
        entity: EntityId,
        item: String,
        quantity: u32,
    },
    SetCurrency {
        entity: EntityId,
        currency: dnd_rules::items::Currency,
    },
    SaveScript {
        script: Box<crate::scripts::ScriptedEvent>,
    },
    DeleteScript {
        id: u64,
    },
    FireScript {
        id: u64,
        actor: Option<EntityId>,
    },
    ClearDrawings {
        room: RoomId,
        temporary_only: bool,
    },
    DeleteDrawing {
        room: RoomId,
        id: u64,
    },
    SetPlayersCanDraw {
        allowed: bool,
    },
    StartSession,
    ViewRoom {
        room: RoomId,
    },
    CreateRoom {
        name: String,
        width: i32,
        height: i32,
        ambient: AmbientLight,
    },
    RenameRoom {
        room: RoomId,
        name: String,
        description: String,
    },
    ConnectRooms {
        from: RoomId,
        to: RoomId,
        one_way: bool,
    },
    SetEdge {
        edge: EdgeId,
        locked: bool,
        secret: bool,
        arrival: Option<Cell>,
    },
    DeleteEdge {
        edge: EdgeId,
    },
    PaintTiles {
        room: RoomId,
        layer: LayerKind,
        cells: Vec<Cell>,
        asset: Option<String>,
    },
    FillRect {
        room: RoomId,
        layer: LayerKind,
        a: Cell,
        b: Cell,
        asset: Option<String>,
    },
    FloodFill {
        room: RoomId,
        layer: LayerKind,
        start: Cell,
        asset: Option<String>,
    },
    SetLayer {
        room: RoomId,
        layer: LayerKind,
        visible: bool,
        locked: bool,
    },
    PlaceObject {
        room: RoomId,
        asset: String,
        cell: Cell,
        hidden: bool,
    },
    RemoveObject {
        room: RoomId,
        object: ObjectId,
    },
    SetObject {
        room: RoomId,
        object: ObjectId,
        hidden: bool,
        open: bool,
    },
    SetAmbient {
        room: RoomId,
        ambient: AmbientLight,
    },
    SetEntryPoint {
        room: RoomId,
        cell: Cell,
    },
    AddLight {
        room: RoomId,
        cell: Cell,
        bright_ft: u32,
        dim_ft: u32,
    },
    ClearLights {
        room: RoomId,
    },
    Undo {
        room: RoomId,
    },
    Redo {
        room: RoomId,
    },
    SpawnMonster {
        template: String,
        room: RoomId,
        cell: Cell,
        hidden: bool,
    },
    RemoveCreature {
        entity: EntityId,
    },
    SetCreatureHidden {
        entity: EntityId,
        hidden: bool,
    },
    /// Moves a creature without movement rules (DM override).
    Teleport {
        entity: EntityId,
        cell: Cell,
    },
    AdjustHp {
        entity: EntityId,
        delta: i32,
    },
    SetCondition {
        entity: EntityId,
        condition: Condition,
        present: bool,
    },
    StartCombat {
        surprised: Vec<EntityId>,
    },
    EndCombat,
    /// Forces the current turn to end (skip).
    NextTurn,
    ReorderInitiative {
        order: Vec<EntityId>,
    },
    TransitionParty {
        to: RoomId,
        override_rules: bool,
    },
    RevealRoom {
        room: RoomId,
        explored: bool,
    },
    SetExplorationMovement {
        allowed: bool,
    },
}

impl Command {
    /// Map-editing commands snapshot the room for undo.
    pub fn edited_room(&self) -> Option<RoomId> {
        match self {
            Command::Dm(
                DmCommand::PaintTiles { room, .. }
                | DmCommand::FillRect { room, .. }
                | DmCommand::FloodFill { room, .. }
                | DmCommand::PlaceObject { room, .. }
                | DmCommand::RemoveObject { room, .. }
                | DmCommand::SetObject { room, .. }
                | DmCommand::SetAmbient { room, .. }
                | DmCommand::SetLayer { room, .. }
                | DmCommand::AddLight { room, .. }
                | DmCommand::ClearLights { room }
                | DmCommand::UpdateObject { room, .. }
                | DmCommand::AddObjects { room, .. }
                | DmCommand::RemoveObjects { room, .. }
                | DmCommand::MoveObjects { room, .. }
                | DmCommand::MoveLayer { room, .. },
            ) => Some(*room),
            _ => None,
        }
    }
}
