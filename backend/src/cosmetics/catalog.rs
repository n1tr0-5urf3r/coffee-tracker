//! Static catalog of cosmetic items. Purely visual: nothing here affects prices,
//! payments or the Stempelkarte. The frontend renders items by `id`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Frame,
    Avatar,
    Cup,
    Stamp,
    Title,
    Skin,
    Collectible,
}

impl Slot {
    pub const EQUIPPABLE: [Slot; 6] = [
        Slot::Frame,
        Slot::Avatar,
        Slot::Cup,
        Slot::Stamp,
        Slot::Title,
        Slot::Skin,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Frame => "frame",
            Slot::Avatar => "avatar",
            Slot::Cup => "cup",
            Slot::Stamp => "stamp",
            Slot::Title => "title",
            Slot::Skin => "skin",
            Slot::Collectible => "collectible",
        }
    }

    pub fn equippable(self) -> bool {
        self != Slot::Collectible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 4] = [
        Rarity::Common,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    /// Drop weight out of [`TOTAL_WEIGHT`].
    pub fn weight(self) -> u64 {
        match self {
            Rarity::Common => 6_400,
            Rarity::Rare => 2_500,
            Rarity::Epic => 900,
            Rarity::Legendary => 200,
        }
    }

    /// Beans awarded when a lootbox drops an item the user already owns.
    pub fn duplicate_beans(self) -> i64 {
        match self {
            Rarity::Common => 10,
            Rarity::Rare => 25,
            Rarity::Epic => 60,
            Rarity::Legendary => 150,
        }
    }

    /// Beans needed to craft an item of this rarity directly.
    pub fn craft_cost(self) -> i64 {
        match self {
            Rarity::Common => 60,
            Rarity::Rare => 150,
            Rarity::Epic => 400,
            Rarity::Legendary => 1_000,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
        }
    }
}

pub const TOTAL_WEIGHT: u64 = 10_000;

#[derive(Debug)]
pub struct CosmeticDef {
    pub id: &'static str,
    pub slot: Slot,
    pub rarity: Rarity,
    pub name: &'static str,
    pub description: &'static str,
    /// Emoji fallback the frontend can show when it has no dedicated artwork.
    pub glyph: Option<&'static str>,
    pub craftable: bool,
    /// Artwork that must not be published (team photos, third-party stickers).
    /// It lives in the gitignored `<repo>/cosmetics_private/` folder, outside the
    /// Docker build context, and is mounted into the frontend container at runtime.
    pub private_artwork: bool,
}

impl CosmeticDef {
    /// Where the image lives, for items with artwork. Public files are static
    /// (`cosmetics/avatars/…`, relative to the frontend base URL); private ones
    /// are served by the backend to members only (`api/v1/cosmetics/artwork/…`).
    pub fn artwork_path(&self) -> Option<String> {
        (self.slot == Slot::Avatar).then(|| {
            if self.private_artwork {
                format!("api/v1/cosmetics/artwork/{}", self.id)
            } else {
                format!("cosmetics/avatars/{}.webp", self.id)
            }
        })
    }

    /// Guests never see (or roll) items with private artwork.
    pub fn visible_to(&self, is_member: bool) -> bool {
        is_member || !self.private_artwork
    }
}

const fn item(
    id: &'static str,
    slot: Slot,
    rarity: Rarity,
    name: &'static str,
    description: &'static str,
    glyph: Option<&'static str>,
) -> CosmeticDef {
    CosmeticDef {
        id,
        slot,
        rarity,
        name,
        description,
        glyph,
        craftable: true,
        private_artwork: false,
    }
}

const fn private_item(
    id: &'static str,
    slot: Slot,
    rarity: Rarity,
    name: &'static str,
    description: &'static str,
    glyph: Option<&'static str>,
) -> CosmeticDef {
    CosmeticDef {
        private_artwork: true,
        ..item(id, slot, rarity, name, description, glyph)
    }
}

use Rarity::{Common, Epic, Legendary, Rare};
use Slot::{Avatar, Collectible, Cup, Frame, Skin, Stamp, Title};

pub const CATALOG: &[CosmeticDef] = &[
    // Frames around the profile picture.
    item(
        "frame_crema",
        Frame,
        Common,
        "Crema Ring",
        "A smooth, crema-coloured ring.",
        None,
    ),
    item(
        "frame_bean",
        Frame,
        Common,
        "Bean Border",
        "Roasted beans all the way round.",
        None,
    ),
    item(
        "frame_steam",
        Frame,
        Rare,
        "Steam Halo",
        "Wisps of steam drift around you.",
        None,
    ),
    item(
        "frame_neon",
        Frame,
        Rare,
        "Neon Espresso",
        "Glows like a café sign at 2 am.",
        None,
    ),
    item(
        "frame_gold",
        Frame,
        Epic,
        "Golden Portafilter",
        "Polished brass for serious baristas.",
        None,
    ),
    item(
        "frame_holo",
        Frame,
        Legendary,
        "Holo Roast",
        "Shimmering holographic foil.",
        None,
    ),
    // Communication Networks lab specials.
    item(
        "frame_ethernet",
        Frame,
        Common,
        "Ethernet Cable",
        "Striped like a T568B patch cable.",
        None,
    ),
    item(
        "frame_wifi",
        Frame,
        Rare,
        "Wi-Fi Rings",
        "Full bars, always.",
        None,
    ),
    item(
        "frame_fiber",
        Frame,
        Epic,
        "Fiber Optic",
        "A packet of light circling at 200,000 km/s.",
        None,
    ),
    // Profile pictures.
    item(
        "avatar_bean",
        Avatar,
        Common,
        "Bean Buddy",
        "A cheerful roasted bean.",
        Some("🫘"),
    ),
    item(
        "avatar_latte_cat",
        Avatar,
        Common,
        "Latte Cat",
        "Sleepy, but never without foam art.",
        Some("🐱"),
    ),
    item(
        "avatar_night_owl",
        Avatar,
        Common,
        "Night Owl",
        "Deadlines are a lifestyle.",
        Some("🦉"),
    ),
    item(
        "avatar_decaf_sloth",
        Avatar,
        Rare,
        "Decaf Sloth",
        "Takes it slow. Very slow.",
        Some("🦥"),
    ),
    item(
        "avatar_barista_bot",
        Avatar,
        Rare,
        "Barista Bot",
        "Beep boop, flat white?",
        Some("🤖"),
    ),
    item(
        "avatar_lab_axolotl",
        Avatar,
        Rare,
        "Lab Axolotl",
        "Regenerates after every all-nighter.",
        Some("🧪"),
    ),
    item(
        "avatar_frappe_unicorn",
        Avatar,
        Epic,
        "Frappé Unicorn",
        "Whipped cream mane, sparkles included.",
        Some("🦄"),
    ),
    item(
        "avatar_espresso_wyvern",
        Avatar,
        Epic,
        "Espresso Wyvern",
        "Breathes steam instead of fire.",
        Some("🐉"),
    ),
    item(
        "avatar_royal_roast",
        Avatar,
        Legendary,
        "Royal Roast",
        "All hail the crowned cup.",
        Some("👑"),
    ),
    // Communication Networks lab specials.
    item(
        "avatar_router_raccoon",
        Avatar,
        Common,
        "Router Raccoon",
        "Steals your packets, returns them with coffee.",
        Some("🦝"),
    ),
    item(
        "avatar_latency_snail",
        Avatar,
        Common,
        "Latency Snail",
        "Your request is important to us.",
        Some("🐌"),
    ),
    item(
        "avatar_carrier_pigeon",
        Avatar,
        Rare,
        "Carrier Pigeon",
        "RFC 1149 compliant. High latency, great throughput.",
        Some("🕊️"),
    ),
    item(
        "avatar_syn_ack",
        Avatar,
        Rare,
        "SYN & ACK",
        "The three-way handshake, minus one cup.",
        Some("🤝"),
    ),
    item(
        "avatar_satellite_cat",
        Avatar,
        Rare,
        "Satellite Cat",
        "550 km up and still waiting for the coffee.",
        Some("🛰️"),
    ),
    item(
        "avatar_fiber_jellyfish",
        Avatar,
        Epic,
        "Fiber Jellyfish",
        "Total internal reflection, total cuteness.",
        Some("🪼"),
    ),
    item(
        "avatar_packet_ghost",
        Avatar,
        Epic,
        "Packet Loss Ghost",
        "It was sent. It never arrived.",
        Some("👻"),
    ),
    item(
        "avatar_wifi_wizard",
        Avatar,
        Epic,
        "Wi-Fi Wizard",
        "Turns walls into signal strength.",
        Some("🧙"),
    ),
    item(
        "avatar_bgp_dragon",
        Avatar,
        Legendary,
        "BGP Dragon",
        "Routes the whole internet. Usually correctly.",
        Some("🐉"),
    ),
    private_item(
        "avatar_snaggs_brb",
        Avatar,
        Rare,
        "BRB Snaggs",
        "Back in five minutes. Probably.",
        Some("📝"),
    ),
    private_item(
        "avatar_snaggs_oh_no",
        Avatar,
        Rare,
        "Oh-no Snaggs",
        "Just looked at the coffee tab.",
        Some("🙈"),
    ),
    private_item(
        "avatar_snaggs_smooth",
        Avatar,
        Rare,
        "Smooth Snaggs",
        "Spilled the coffee. Smoothly.",
        Some("☕"),
    ),
    private_item(
        "avatar_snaggs_party",
        Avatar,
        Epic,
        "Party Snaggs",
        "Too cool for decaf.",
        Some("😎"),
    ),
    private_item(
        "avatar_snaggs_help",
        Avatar,
        Epic,
        "Deadline Snaggs",
        "When the build fails five minutes before the deadline.",
        Some("😱"),
    ),
    private_item(
        "avatar_snaggs_monday",
        Avatar,
        Legendary,
        "Monday Snaggs",
        "Before the first cup. Do not approach.",
        Some("🥱"),
    ),
    private_item(
        "avatar_mannequin",
        Avatar,
        Rare,
        "The Night Shift",
        "Has not left the lab since 2024.",
        Some("🧍"),
    ),
    private_item(
        "avatar_spaghetti",
        Avatar,
        Rare,
        "Snack Researcher",
        "Thinking really hard about lunch.",
        Some("🥢"),
    ),
    private_item(
        "avatar_hacker",
        Avatar,
        Epic,
        "The Hacker",
        "I'm in.",
        Some("💻"),
    ),
    private_item(
        "avatar_tinfoil",
        Avatar,
        Epic,
        "Tinfoil Hat",
        "Shielded from the coffee machine's Wi-Fi.",
        Some("🛸"),
    ),
    private_item(
        "avatar_beekeeper",
        Avatar,
        Epic,
        "The Beekeeper",
        "Thumbs up from the hive. Ask about the honey.",
        Some("🐝"),
    ),
    private_item(
        "avatar_etienne",
        Avatar,
        Legendary,
        "Étienné Zinqué",
        "Lead NetEm Developer. Bonjour! Do you have a moment to talk about honey?",
        Some("🥸"),
    ),
    // Cup shown on the Log page.
    item(
        "cup_mug",
        Cup,
        Common,
        "Office Mug",
        "The trusty mug from the cupboard.",
        Some("☕"),
    ),
    item(
        "cup_togo",
        Cup,
        Common,
        "To-Go Cup",
        "Always on the move.",
        Some("🥤"),
    ),
    item(
        "cup_teapot",
        Cup,
        Rare,
        "Teapot",
        "For the tea people among us.",
        Some("🫖"),
    ),
    item(
        "cup_coconut",
        Cup,
        Rare,
        "Coconut Shell",
        "Tropical vibes at the desk.",
        Some("🥥"),
    ),
    item(
        "cup_trophy",
        Cup,
        Epic,
        "Trophy Cup",
        "Every coffee is a win.",
        Some("🏆"),
    ),
    item(
        "cup_potion",
        Cup,
        Legendary,
        "Lab Potion",
        "Straight from the lab bench. Do not pipette by mouth.",
        Some("🧪"),
    ),
    // Communication Networks lab specials.
    item(
        "cup_dish",
        Cup,
        Rare,
        "Satellite Dish",
        "Receives coffee from orbit.",
        Some("📡"),
    ),
    item(
        "cup_satellite",
        Cup,
        Epic,
        "Coffee Satellite",
        "Geostationary above your desk.",
        Some("🛰️"),
    ),
    // Stamp styles on the Stempelkarte.
    item(
        "stamp_mint",
        Stamp,
        Common,
        "Mint Ink",
        "Fresh green stamps.",
        None,
    ),
    item(
        "stamp_sky",
        Stamp,
        Common,
        "Blueprint Ink",
        "Stamps in engineering blue.",
        None,
    ),
    item(
        "stamp_double",
        Stamp,
        Rare,
        "Double Ring",
        "An extra ring for extra officialness.",
        None,
    ),
    item(
        "stamp_gold",
        Stamp,
        Rare,
        "Gold Foil",
        "Hot-pressed gold foil stamps.",
        None,
    ),
    item(
        "stamp_wax",
        Stamp,
        Epic,
        "Wax Seal",
        "Sealed like a royal letter.",
        None,
    ),
    item(
        "stamp_rainbow",
        Stamp,
        Legendary,
        "Rainbow Ink",
        "Every stamp a different colour.",
        None,
    ),
    // Communication Networks lab specials.
    item(
        "stamp_checksum",
        Stamp,
        Rare,
        "Checksum Green",
        "Every stamp verified. CRC OK.",
        None,
    ),
    // Titles shown next to the name.
    item(
        "title_regular",
        Title,
        Common,
        "Regular",
        "Known at the machine.",
        None,
    ),
    item(
        "title_intern",
        Title,
        Common,
        "Caffeine Intern",
        "Still learning the grind.",
        None,
    ),
    item(
        "title_bean_counter",
        Title,
        Rare,
        "Bean Counter",
        "Every cup accounted for.",
        None,
    ),
    item(
        "title_barista",
        Title,
        Rare,
        "Barista",
        "Knows the difference between a latte and a flat white.",
        None,
    ),
    item(
        "title_night_shift",
        Title,
        Rare,
        "Night Shift",
        "Brews after dark.",
        None,
    ),
    item(
        "title_espresso_overlord",
        Title,
        Epic,
        "Espresso Overlord",
        "Rules the machine.",
        None,
    ),
    item(
        "title_caffeine_wizard",
        Title,
        Epic,
        "Caffeine Wizard",
        "Turns beans into productivity.",
        None,
    ),
    item(
        "title_living_legend",
        Title,
        Legendary,
        "Living Legend of the Lab",
        "They tell stories about you.",
        None,
    ),
    item(
        "title_vibe_coder",
        Title,
        Rare,
        "Vibe Coder",
        "Ships it if it feels right.",
        None,
    ),
    item(
        "title_sofazerstoerer",
        Title,
        Epic,
        "Der Sofazerstörer",
        "Das Sofa hatte keine Chance.",
        None,
    ),
    item(
        "title_spaghettini_assassini",
        Title,
        Epic,
        "Spaghettini Assassini",
        "Silent, deadly, al dente.",
        None,
    ),
    item(
        "title_special_mission_operator",
        Title,
        Legendary,
        "Special Mission Operator",
        "Mission details are classified.",
        None,
    ),
    // Communication Networks lab specials.
    item(
        "title_packet_sniffer",
        Title,
        Common,
        "Packet Sniffer",
        "Smells the coffee before it is brewed.",
        None,
    ),
    item(
        "title_ping_master",
        Title,
        Common,
        "Ping Master",
        "64 bytes from coffee: icmp_seq=1 ttl=64.",
        None,
    ),
    item(
        "title_ttl_expired",
        Title,
        Rare,
        "TTL Expired",
        "Too many hops before the first cup.",
        None,
    ),
    item(
        "title_three_way_handshaker",
        Title,
        Rare,
        "Three-Way Handshaker",
        "SYN, SYN-ACK, ACK. Then coffee.",
        None,
    ),
    item(
        "title_layer_8",
        Title,
        Rare,
        "Layer 8 Problem",
        "The issue is between chair and keyboard.",
        None,
    ),
    item(
        "title_bufferbloat_survivor",
        Title,
        Epic,
        "Bufferbloat Survivor",
        "Queued, delayed, but delivered.",
        None,
    ),
    item(
        "title_bgp_whisperer",
        Title,
        Epic,
        "BGP Whisperer",
        "Knows which AS took the last coffee.",
        None,
    ),
    item(
        "title_keeper_root_servers",
        Title,
        Legendary,
        "Keeper of the Root Servers",
        "All thirteen of them answer to you.",
        None,
    ),
    // Whole-UI skins (surfaces only; the brand red stays).
    item(
        "skin_oat",
        Skin,
        Common,
        "Oat Milk",
        "Soft, creamy surfaces.",
        None,
    ),
    item(
        "skin_matcha",
        Skin,
        Rare,
        "Matcha",
        "A calm green tint.",
        None,
    ),
    item(
        "skin_kraft",
        Skin,
        Rare,
        "Kraft Paper",
        "Like the old paper tally sheet.",
        None,
    ),
    item(
        "skin_mocha",
        Skin,
        Epic,
        "Mocha",
        "Deep chocolate browns.",
        None,
    ),
    item(
        "skin_terminal",
        Skin,
        Legendary,
        "Lab Terminal",
        "Monospace everything. Very hacker.",
        None,
    ),
    // Communication Networks lab specials.
    item(
        "skin_noc",
        Skin,
        Epic,
        "Network Operations Center",
        "Midnight blue, all links green.",
        None,
    ),
    // Collectibles (not equippable).
    CosmeticDef {
        id: "collectible_kn_museum_ticket",
        slot: Collectible,
        rarity: Legendary,
        name: "KN Museum Ticket",
        description: "Entry ticket to the KN museum, face value 5 €. Collectible only — no real value.",
        glyph: Some("🎟️"),
        craftable: false,
        private_artwork: false,
    },
    CosmeticDef {
        id: "collectible_golden_patch_cable",
        slot: Collectible,
        rarity: Epic,
        name: "Golden Patch Cable",
        description: "Gold-plated Cat 8, one metre. Improves latency by exactly 0 ms.",
        glyph: Some("🔌"),
        craftable: true,
        private_artwork: false,
    },
];

pub fn find(id: &str) -> Option<&'static CosmeticDef> {
    CATALOG.iter().find(|item| item.id == id)
}

pub fn rarity_for_roll(roll: u64) -> Rarity {
    let mut remaining = roll % TOTAL_WEIGHT;
    for rarity in Rarity::ALL {
        if remaining < rarity.weight() {
            return rarity;
        }
        remaining -= rarity.weight();
    }
    Rarity::Common
}

/// Picks an item: `rarity_roll` chooses the rarity tier by weight, `item_roll`
/// picks uniformly within that tier (among the items the user may see).
pub fn roll_item(rarity_roll: u64, item_roll: u64, is_member: bool) -> &'static CosmeticDef {
    let rarity = rarity_for_roll(rarity_roll);
    let pool: Vec<&'static CosmeticDef> = CATALOG
        .iter()
        .filter(|item| item.rarity == rarity && item.visible_to(is_member))
        .collect();
    pool[(item_roll % pool.len() as u64) as usize]
}

pub fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    rand::fill(&mut bytes);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn rarity_weights_sum_to_total() {
        let sum: u64 = Rarity::ALL.iter().map(|rarity| rarity.weight()).sum();
        assert_eq!(sum, TOTAL_WEIGHT);
    }

    #[test]
    fn rarity_boundaries_follow_weights() {
        assert_eq!(rarity_for_roll(0), Rarity::Common);
        assert_eq!(rarity_for_roll(6_399), Rarity::Common);
        assert_eq!(rarity_for_roll(6_400), Rarity::Rare);
        assert_eq!(rarity_for_roll(8_899), Rarity::Rare);
        assert_eq!(rarity_for_roll(8_900), Rarity::Epic);
        assert_eq!(rarity_for_roll(9_799), Rarity::Epic);
        assert_eq!(rarity_for_roll(9_800), Rarity::Legendary);
        assert_eq!(rarity_for_roll(9_999), Rarity::Legendary);
        assert_eq!(rarity_for_roll(10_000), Rarity::Common);
    }

    #[test]
    fn every_tier_has_items_and_ids_are_unique() {
        for rarity in Rarity::ALL {
            assert!(CATALOG.iter().any(|item| item.rarity == rarity));
        }
        let ids: HashSet<_> = CATALOG.iter().map(|item| item.id).collect();
        assert_eq!(ids.len(), CATALOG.len());
    }

    #[test]
    fn every_legendary_is_reachable() {
        let legendary_count = CATALOG
            .iter()
            .filter(|item| item.rarity == Legendary)
            .count() as u64;
        let pulled: HashSet<_> = (0..legendary_count)
            .map(|item_roll| roll_item(9_900, item_roll, true).id)
            .collect();
        assert_eq!(pulled.len() as u64, legendary_count);
        assert!(pulled.contains("collectible_kn_museum_ticket"));
    }

    #[test]
    fn every_public_avatar_ships_artwork() {
        // Private artwork is not in git (see `CosmeticDef::private_artwork`), so
        // only public files can be required here.
        let public_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../frontend/coffee-tracker/public");
        for item in CATALOG.iter().filter(|item| item.slot == Slot::Avatar) {
            let path = public_dir.join(item.artwork_path().unwrap());
            if item.private_artwork {
                // Served by the backend from `cosmetics_private/`, which is not in git.
                assert!(
                    item.artwork_path()
                        .unwrap()
                        .starts_with("api/v1/cosmetics/artwork/")
                );
                continue;
            }
            assert!(path.exists(), "missing avatar artwork {}", path.display());
        }
    }

    #[test]
    fn guests_never_roll_private_artwork() {
        for rarity_roll in [0, 7_000, 9_000, 9_900] {
            for item_roll in 0..64 {
                assert!(!roll_item(rarity_roll, item_roll, false).private_artwork);
            }
        }
        for rarity in Rarity::ALL {
            assert!(
                CATALOG
                    .iter()
                    .any(|item| item.rarity == rarity && item.visible_to(false))
            );
        }
    }

    #[test]
    fn museum_ticket_cannot_be_crafted() {
        assert!(!find("collectible_kn_museum_ticket").unwrap().craftable);
        assert!(!Slot::Collectible.equippable());
    }
}
