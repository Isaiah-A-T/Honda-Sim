//! Material families (for colour) and the placeholder catalog used when the sim doesn't send its own
//! materials. `strength` is a rough relative figure (stock = 1), not engineering data.

pub struct Family {
    pub id: &'static str,
    pub label: &'static str,
    pub color: [u8; 3],
}

pub const FAMILIES: &[Family] = &[
    Family { id: "aluminum", label: "Aluminium", color: [0x4f, 0x97, 0xe3] },
    Family { id: "steel", label: "Steel", color: [0x8d, 0x97, 0xa4] },
    Family { id: "chromoly", label: "Chromoly 4340", color: [0xe0, 0x84, 0x3f] },
    Family { id: "titanium", label: "Titanium", color: [0xa6, 0x7a, 0xd6] },
    Family { id: "nickel", label: "Inconel", color: [0xd4, 0xb1, 0x3a] },
    Family { id: "iron", label: "Iron", color: [0x7a, 0x66, 0x56] },
    Family { id: "stainless", label: "Stainless", color: [0x3f, 0xb5, 0x9a] },
    Family { id: "friction", label: "Friction material", color: [0xc7, 0x5b, 0x7a] },
];

pub fn family(id: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|f| f.id == id)
}

pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    pub family: &'static str,
    pub strength: f32,
}

pub struct PartCatalog {
    pub key: &'static str,
    pub label: &'static str,
    pub options: &'static [Choice],
}

macro_rules! c {
    ($id:expr, $label:expr, $fam:expr, $s:expr) => {
        Choice { id: $id, label: $label, family: $fam, strength: $s }
    };
}

pub const CATALOG: &[PartCatalog] = &[
    PartCatalog { key: "pistons", label: "Pistons", options: &[
        c!("stock-cast", "Stock cast", "aluminum", 1.0),
        c!("forged-4032", "Forged 4032", "aluminum", 1.35),
        c!("forged-2618", "Forged 2618", "aluminum", 1.5),
    ] },
    PartCatalog { key: "rods", label: "Connecting rods", options: &[
        c!("stock", "Stock powder-forged steel", "steel", 1.0),
        c!("4340-h", "4340 H-beam", "chromoly", 1.7),
        c!("4340-i", "4340 I-beam", "chromoly", 1.8),
        c!("ti", "Titanium", "titanium", 1.6),
    ] },
    PartCatalog { key: "crank", label: "Crankshaft", options: &[
        c!("stock", "Stock forged steel", "steel", 1.0),
        c!("billet", "Billet 4340", "chromoly", 1.4),
    ] },
    PartCatalog { key: "valves", label: "Valves", options: &[
        c!("stock", "Stock steel", "steel", 1.0),
        c!("ss", "Stainless", "stainless", 1.1),
        c!("inconel", "Inconel", "nickel", 1.3),
        c!("ti", "Titanium (lighter, less float)", "titanium", 1.25),
    ] },
    PartCatalog { key: "cams", label: "Camshafts", options: &[
        c!("stock", "Stock chilled iron", "iron", 1.0),
        c!("billet", "Billet steel", "steel", 1.2),
    ] },
    PartCatalog { key: "block", label: "Block / sleeves", options: &[
        c!("stock", "Stock aluminium, cast-in iron liners", "aluminum", 1.0),
        c!("sleeved", "Ductile iron sleeves", "iron", 1.4),
        c!("closed", "Closed-deck sleeves", "iron", 1.6),
    ] },
    PartCatalog { key: "head", label: "Cylinder head", options: &[c!("stock", "Stock aluminium", "aluminum", 1.0)] },
    PartCatalog { key: "exhaust", label: "Exhaust manifold", options: &[
        c!("mild", "Mild steel", "steel", 1.0),
        c!("304", "304 stainless", "stainless", 1.2),
        c!("321", "321 stainless", "stainless", 1.3),
        c!("inconel", "Inconel", "nickel", 1.6),
    ] },
    PartCatalog { key: "turbo", label: "Turbo housings", options: &[
        c!("cast-iron", "Cast iron", "iron", 1.0),
        c!("ni-resist", "Ni-resist", "nickel", 1.2),
    ] },
    PartCatalog { key: "flywheel", label: "Flywheel", options: &[
        c!("stock", "Stock steel", "steel", 1.0),
        c!("chromoly", "Lightweight chromoly", "chromoly", 1.2),
        c!("alu", "Aluminium", "aluminum", 0.9),
    ] },
    PartCatalog { key: "clutch", label: "Clutch", options: &[
        c!("stock", "Stock organic", "friction", 1.0),
        c!("stage2", "Stage 2 organic", "friction", 1.3),
        c!("stage3", "Stage 3 ceramic puck", "friction", 1.6),
    ] },
];

pub fn catalog(key: &str) -> Option<&'static PartCatalog> {
    CATALOG.iter().find(|p| p.key == key)
}
