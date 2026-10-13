//! Bottom end: the block (src/engines.js) with its own crank, rods and pistons, or the aftermarket
//! crank (or stroker kit), rods and pistons fitted in place of them, whether they fit together, and the
//! bore, stroke, rod length, compression and displacement they add up to (src/parts.js).
//! Every aftermarket part is a real part from docs/parts-catalog.json.

use crate::engines::{engine_setup, Block, Fit, Head, OilPump};
use crate::tune::{EngineSel, Tune};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const K20_DECK: f64 = 212.0;
const K24_DECK: f64 = 231.5;
const STOCK_PISTON_KG: f64 = 0.290;
const STOCK_ROD_RECIP_KG: f64 = 0.160;
const ROD_RECIP_SHARE: f64 = 0.32;
const MAX_PROTRUSION_MM: f64 = 0.3;
const MAX_OVERBORE_MM: f64 = 1.0;
const LEGACY_K24: &str = "honda-k24-99-hybrid";

fn cc(bore_mm: f64, stroke_mm: f64) -> f64 {
    std::f64::consts::PI / 4.0 * bore_mm.powi(2) * stroke_mm / 1000.0
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Piston {
    pub id: String,
    pub brand: String,
    pub part_number: String,
    pub name: String,
    pub bore_mm: f64,
    pub compression_ratio: f64,
    pub compression_height_mm: f64,
    pub pin_dia_mm: f64,
    pub weight_g: Option<f64>,
    pub for_stroke_mm: f64,
    pub for_rod_length_mm: f64,
    pub for_deck_mm: f64,
    pub material: Option<&'static str>,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rod {
    pub id: String,
    pub brand: String,
    pub part_number: String,
    pub name: String,
    pub length_mm: Option<f64>,
    pub big_end_mm: f64,
    pub pin_dia_mm: f64,
    pub weight_g: Option<f64>,
    pub material_id: Option<&'static str>,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KitPiston {
    pub name: String,
    pub bore_mm: f64,
    pub compression_ratio: Option<f64>,
    pub clearance_mm: f64,
    pub pin_mm: Option<f64>,
    pub alloy: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crank {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub stroke_mm: f64,
    pub rod_length_mm: Option<f64>,
    pub deck_mm: f64,
    pub housing_mm: Option<f64>,
    pub material: Option<&'static str>,
    pub family: &'static str,
    pub rod_locked: Option<String>,
    pub kit_piston: Option<KitPiston>,
    pub kit_rod: Option<String>,
    pub estimated: BTreeMap<String, String>,
    pub sources: Vec<String>,
}

pub struct Catalog {
    pub cranks: Vec<Crank>,
    pub pistons: Vec<Piston>,
    pub rods: Vec<Rod>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}
fn sources(v: &Value) -> Vec<String> {
    v.get("sources").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|u| u.as_str().map(String::from)).collect()).unwrap_or_default()
}
fn piston_material(alloy: &str) -> Option<&'static str> {
    if alloy.contains("4032") { Some("piston4032") } else if alloy.contains("2618") { Some("piston2618") } else { None }
}
fn rod_material(m: &str) -> Option<&'static str> {
    let l = m.to_lowercase();
    if l.contains("4330") || l.contains("4340") || l.contains("chromoly") { Some("rod4340") } else { None }
}
fn fmt_num(x: f64) -> String {
    if x.fract() == 0.0 { format!("{}", x as i64) } else { format!("{}", x) }
}

pub fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(|| {
        let data: Value = serde_json::from_str(include_str!("../data/parts-catalog.json")).expect("parts catalog");
        let stock_sources = sources(&data["stock"]);
        let crank_json = |id: &str| data["cranks"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap().clone();
        let rod_json = |id: &str| data["rods"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap().clone();

        let mut pistons = vec![Piston {
            id: "stock".into(), brand: "Honda".into(), part_number: "OEM".into(), name: "Stock K20Z3 pistons (86 mm, 11.0:1)".into(),
            bore_mm: 86.0, compression_ratio: 11.0, compression_height_mm: 30.0, pin_dia_mm: 22.0, weight_g: None,
            for_stroke_mm: 86.0, for_rod_length_mm: 139.0, for_deck_mm: K20_DECK, material: None, sources: stock_sources.clone(),
        }];
        for p in data["pistons"].as_array().unwrap() {
            let for_stroke = f(p, "forStroke_mm").unwrap_or(86.0);
            pistons.push(Piston {
                id: s(p, "id"), brand: s(p, "brand"), part_number: s(p, "partNumber"),
                name: format!("{} {} ({} mm, {}:1)", s(p, "brand"), s(p, "partNumber"), fmt_num(f(p, "bore_mm").unwrap()), fmt_num(f(p, "compressionRatio").unwrap())),
                bore_mm: f(p, "bore_mm").unwrap(), compression_ratio: f(p, "compressionRatio").unwrap(),
                compression_height_mm: f(p, "compressionHeight_mm").unwrap(), pin_dia_mm: f(p, "pinDia_mm").unwrap_or(22.0),
                weight_g: f(p, "pistonWeight_g"), for_stroke_mm: for_stroke, for_rod_length_mm: f(p, "forRodLength_mm").unwrap_or(139.0),
                for_deck_mm: if for_stroke == 99.0 { K24_DECK } else { K20_DECK },
                material: piston_material(&s(p, "alloy")), sources: sources(p),
            });
        }
        let mut rods = vec![Rod {
            id: "stock".into(), brand: "Honda".into(), part_number: "OEM".into(), name: "Stock rods (OEM length for the crank)".into(),
            length_mm: None, big_end_mm: 51.0, pin_dia_mm: 22.0, weight_g: None, material_id: None, sources: stock_sources.clone(),
        }];
        for r in data["rods"].as_array().unwrap() {
            let w = f(r, "weight_g");
            rods.push(Rod {
                id: s(r, "id"), brand: s(r, "brand"), part_number: s(r, "partNumber"),
                name: format!("{} {} ({} mm, {} g)", s(r, "brand"), s(r, "partNumber"), fmt_num(f(r, "length_mm").unwrap()), w.map(fmt_num).unwrap_or("?".into())),
                length_mm: f(r, "length_mm"), big_end_mm: f(r, "bigEnd_mm").unwrap_or(51.0), pin_dia_mm: f(r, "pinDia_mm").unwrap_or(22.0),
                weight_g: w, material_id: rod_material(&s(r, "material")), sources: sources(r),
            });
        }

        let mut cranks = vec![Crank {
            id: "stock".into(), name: "Stock K20Z3 crank (86 mm stroke)".into(), brand: "Honda".into(), stroke_mm: 86.0, rod_length_mm: Some(139.0),
            deck_mm: K20_DECK, housing_mm: Some(51.0), material: None, family: "K20", rod_locked: None, kit_piston: None, kit_rod: None,
            estimated: BTreeMap::new(), sources: stock_sources.clone(),
        }];
        {
            let c = crank_json("bc-k20-92-stroker");
            let r = rod_json("bc-k20-stroker-140");
            let est = [
                ("bore", "BC7048 bore not published; stock 86 mm assumed"),
                ("compressionRatio", "not published; stock combustion chamber volume assumed, so the longer stroke raises it"),
                ("pistonAlloy", "BC7048 alloy not published; 2618 forging assumed"),
                ("pistonWeight", "not listed; stock weight used"),
            ];
            cranks.push(Crank {
                id: s(&c, "id"), name: "Brian Crower 92 mm stroker kit (BC5048 + BC6040 rods + BC7048 pistons)".into(), brand: s(&c, "brand"),
                stroke_mm: f(&c, "stroke_mm").unwrap(), rod_length_mm: f(&c, "rodLength_mm"), deck_mm: K20_DECK, housing_mm: f(&r, "bigEnd_mm"),
                material: Some("crank4340"), family: "K20", rod_locked: Some(s(&r, "id")),
                kit_piston: Some(KitPiston { name: "BC7048 custom pistons".into(), bore_mm: 86.0, compression_ratio: None, clearance_mm: 0.0, pin_mm: f(&r, "pinDia_mm"), alloy: "2618" }),
                kit_rod: None, estimated: est.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), sources: sources(&c),
            });
        }
        let toda = |id: &str, bore: f64, i: usize, extra: Option<&str>| {
            let c = crank_json(id);
            let pick = |k: &str| match &c[k] {
                Value::Array(a) => a.get(i).and_then(|x| x.as_f64()),
                v => v.as_f64(),
            };
            let mut est: BTreeMap<String, String> = [
                ("rodLength", "Toda does not list rod length; 139 mm assumed (only affects piston speed and side load)"),
                ("pistonWeight", "not listed; stock weight scaled by bore area"),
                ("rodWeight", "not listed; stock rod mass used"),
            ].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
            if let Some(x) = extra { est.insert("compressionRatio".into(), x.into()); }
            Crank {
                id: format!("{}-{}", id, fmt_num(bore)), name: format!("Toda {} mm kit, {} mm bore", fmt_num(f(&c, "stroke_mm").unwrap()), fmt_num(bore)), brand: "Toda".into(),
                stroke_mm: f(&c, "stroke_mm").unwrap(), rod_length_mm: None, deck_mm: K20_DECK, housing_mm: None, material: None, family: "K20", rod_locked: None,
                kit_piston: Some(KitPiston { name: "Toda kit pistons".into(), bore_mm: bore, compression_ratio: pick("compressionRatio"), clearance_mm: 0.0, pin_mm: None, alloy: "forged" }),
                kit_rod: Some("Toda kit rods (I-section chromoly)".into()), estimated: est, sources: sources(&c),
            }
        };
        let gk = "Toda quotes this ratio with its GK head; with the stock head it would be lower";
        cranks.push(toda("toda-k20-90.7-hc", 86.0, 0, None));
        cranks.push(toda("toda-k20-90.7-hc", 86.5, 1, None));
        cranks.push(toda("toda-k20-90.7-hc", 87.0, 2, None));
        cranks.push(toda("toda-k20-90.7-lc", 86.5, 0, None));
        cranks.push(toda("toda-k20-84-short", 87.0, 0, Some(gk)));
        cranks.push(toda("toda-k20-84-short", 87.5, 1, Some(gk)));
        Catalog { cranks, pistons, rods }
    })
}

/// The block's own pistons.
pub fn stock_piston(block: &Block) -> Piston {
    Piston {
        id: "stock".into(), brand: "Honda".into(), part_number: "OEM".into(),
        name: format!("Stock {} pistons ({} mm, {:.1}:1)", block.id, fmt_num(block.bore_mm), block.compression),
        bore_mm: block.bore_mm, compression_ratio: block.compression, compression_height_mm: block.compression_height_mm, pin_dia_mm: block.pin_mm,
        weight_g: None, for_stroke_mm: block.stroke_mm, for_rod_length_mm: block.rod_mm, for_deck_mm: block.deck_mm, material: None, sources: block.sources.clone(),
    }
}
/// The block's own crank.
pub fn stock_crank(block: &Block) -> Crank {
    Crank {
        id: "stock".into(), name: format!("Stock {} crank ({} mm stroke)", block.id, fmt_num(block.stroke_mm)), brand: "Honda".into(),
        stroke_mm: block.stroke_mm, rod_length_mm: Some(block.rod_mm), deck_mm: block.deck_mm, housing_mm: Some(block.big_end_mm), material: None,
        family: if block.family == "K24" { "K24" } else { "K20" }, rod_locked: None, kit_piston: None, kit_rod: None,
        estimated: BTreeMap::new(), sources: block.sources.clone(),
    }
}
/// Crank list for a block: its own crank first, then the aftermarket ones.
pub fn cranks_for(block: &Block) -> Vec<Crank> {
    std::iter::once(stock_crank(block)).chain(catalog().cranks[1..].iter().cloned()).collect()
}
/// Piston list for a block: its own pistons first.
pub fn pistons_for(block: &Block) -> Vec<Piston> {
    std::iter::once(stock_piston(block)).chain(catalog().pistons[1..].iter().cloned()).collect()
}

const A3_NOTE: &str = "aftermarket K20 bottom ends don't list the K20A3, whose rods and bearings differ";

/// Why this crank can't go in this block, or None if it fits.
pub fn crank_problem(block: &Block, crank: &Crank) -> Option<String> {
    if crank.id == "stock" {
        return None;
    }
    if !block.aftermarket_bottom_end {
        return Some(A3_NOTE.into());
    }
    if crank.family != block.family {
        return Some(format!("made for the {} block ({} mm deck); the {} deck is {} mm", crank.family, fmt_num(crank.deck_mm), block.id, fmt_num(block.deck_mm)));
    }
    None
}

/// Why this piston can't go in with this crank, or None if it fits.
pub fn piston_problem(crank: &Crank, piston: &Piston, block: &Block) -> Option<String> {
    if crank.kit_piston.is_some() {
        return if piston.id == "stock" { None } else { Some("this kit comes with its own pistons".into()) };
    }
    if piston.id == "stock" {
        return None;
    }
    if !block.aftermarket_bottom_end {
        return Some(A3_NOTE.into());
    }
    if piston.bore_mm < block.bore_mm {
        return Some(format!("{} mm is smaller than the {}'s {} mm bore", fmt_num(piston.bore_mm), block.id, fmt_num(block.bore_mm)));
    }
    let rod_mm = crank.rod_length_mm.unwrap_or(139.0);
    let clearance = crank.deck_mm - (crank.stroke_mm / 2.0 + rod_mm + piston.compression_height_mm);
    if clearance < -MAX_PROTRUSION_MM {
        return Some(format!("sticks out {:.1} mm and hits the head", -clearance));
    }
    if clearance > 3.0 {
        return Some(format!("sits {:.1} mm down the bore", clearance));
    }
    None
}

/// Why this rod can't go in with this crank and piston, or None if it fits.
pub fn rod_problem(crank: &Crank, rod: &Rod, piston: Option<&Piston>, block: &Block) -> Option<String> {
    if crank.kit_rod.is_some() {
        return if rod.id == "stock" { None } else { Some("this kit comes with its own rods".into()) };
    }
    if let Some(locked) = &crank.rod_locked {
        return if &rod.id == locked || rod.id == "stock" { None } else { Some("this kit uses the BC6040 rods".into()) };
    }
    if rod.id == "stock" {
        return None;
    }
    if !block.aftermarket_bottom_end {
        return Some(A3_NOTE.into());
    }
    if rod.length_mm != crank.rod_length_mm {
        return Some(format!("{} mm long; this crank needs {} mm", fmt_num(rod.length_mm.unwrap_or(0.0)), fmt_num(crank.rod_length_mm.unwrap_or(0.0))));
    }
    if Some(rod.big_end_mm) != crank.housing_mm {
        return Some(format!("big end {} mm doesn't fit this crank's {} mm journal", fmt_num(rod.big_end_mm), fmt_num(crank.housing_mm.unwrap_or(0.0))));
    }
    if let Some(p) = piston {
        if rod.pin_dia_mm != p.pin_dia_mm {
            return Some(format!("{} mm pin; pistons use {} mm", fmt_num(rod.pin_dia_mm), fmt_num(p.pin_dia_mm)));
        }
    }
    None
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FittedMaterials {
    pub piston: Option<&'static str>,
    pub rod: Option<&'static str>,
    pub crank: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BottomEnd {
    /// The K20Z3 as it came: block, head, oil pump and parts.
    pub stock: bool,
    /// The K20Z3 block and head with their own parts (the geometry the gas model is calibrated on).
    pub geom_stock: bool,
    pub block: &'static Block,
    pub head: &'static Head,
    pub pump: &'static OilPump,
    pub head_fit: Fit,
    pub pump_fit: Fit,
    pub crank: Crank,
    pub piston: Piston,
    pub rod: Rod,
    pub issues: Vec<String>,
    pub estimated: BTreeMap<String, String>,
    pub bore_mm: f64,
    pub stroke_mm: f64,
    pub rod_mm: f64,
    pub deck_mm: f64,
    pub compression_height_mm: f64,
    pub deck_clearance_mm: f64,
    pub compression: f64,
    pub displacement_l: f64,
    pub piston_kg: f64,
    pub rod_recip_kg: f64,
    pub pin_mm: f64,
    pub big_end_bearing_mm: f64,
    pub requires_sleeves: bool,
    pub piston_name: String,
    pub rod_name: String,
    pub materials: FittedMaterials,
}

/// Everything the engine model needs from the fitted block, head, pump and parts.
pub fn bottom_end(tune: &Tune) -> BottomEnd {
    let cat = catalog();
    let sel = &tune.parts;
    let setup = engine_setup(tune);
    let block = setup.block;
    let (mut head, mut head_fit) = (setup.head, setup.head_fit.clone());
    let mut issues = setup.issues.clone();
    let mut estimated = block.estimated.clone();

    let crank_id = sel.crank.as_deref().map(|c| if c == LEGACY_K24 { "stock" } else { c });
    let mut crank = match crank_id {
        Some(id) if id != "stock" => cat.cranks[1..].iter().find(|c| c.id == id).cloned().unwrap_or_else(|| {
            issues.push(format!("Unknown crank \"{id}\"; using stock."));
            stock_crank(block)
        }),
        _ => stock_crank(block),
    };
    if let Some(why) = crank_problem(block, &crank) {
        issues.push(format!("{} can't be used: {}.", crank.name, why));
        crank = stock_crank(block);
    }
    estimated.extend(crank.estimated.clone());

    let stock_p = stock_piston(block);
    let mut piston = sel.piston.as_ref().filter(|id| id.as_str() != "stock").and_then(|id| cat.pistons.iter().find(|p| &p.id == id)).cloned().unwrap_or(stock_p.clone());
    if crank.kit_piston.is_some() {
        piston = stock_p.clone();
    } else if let Some(why) = piston_problem(&crank, &piston, block) {
        issues.push(format!("{} can't be used: {}.", piston.name, why));
        piston = stock_p.clone();
    }
    let rod_id = crank.rod_locked.clone().or(sel.rod.clone());
    let mut rod = rod_id.as_ref().and_then(|id| cat.rods.iter().find(|r| &r.id == id)).unwrap_or(&cat.rods[0]).clone();
    if let Some(why) = rod_problem(&crank, &rod, if crank.kit_piston.is_some() { None } else { Some(&piston) }, block) {
        issues.push(format!("{} can't be used: {}.", rod.name, why));
        rod = match &crank.rod_locked {
            Some(id) => cat.rods.iter().find(|r| &r.id == id).unwrap().clone(),
            None => cat.rods[0].clone(),
        };
    }

    // Stock K24 pistons under a K20 head: A4 pistons risk hitting the valves, A8 pistons hit them.
    let k20_on_k24 = head.family == "K20" && block.family == "K24";
    if k20_on_k24 && piston.id == "stock" && block.pistons_vs_k20_head.as_deref() == Some("contact") {
        issues.push(format!("Stock {} pistons hit the {}'s valves; fit aftermarket pistons. Using the {}'s own head.", block.id, head.name, block.id));
        head = engine_setup(&Tune { engine: EngineSel { block: Some(block.id.clone()), ..Default::default() }, ..Tune::empty() }).head;
        head_fit = Fit { works: "yes".into(), needs: vec![], note: "This block's own head.".into(), sources: vec![] };
    } else if k20_on_k24 && piston.id == "stock" && block.pistons_vs_k20_head.as_deref() == Some("risk") {
        issues.push(format!("Stock {} pistons can touch the {}'s valves; the build guides use K24A1/A2 or aftermarket pistons.", block.id, head.name));
    }

    let stroke_mm = crank.stroke_mm;
    let rod_mm = rod.length_mm.or(crank.rod_length_mm).unwrap_or(139.0);
    let block_vc = cc(block.bore_mm, block.stroke_mm) / (block.compression - 1.0);
    let (bore_mm, compression, piston_kg, ch_mm, piston_mat, pin_mm);
    if let Some(k) = &crank.kit_piston {
        bore_mm = k.bore_mm;
        ch_mm = crank.deck_mm - stroke_mm / 2.0 - rod_mm - k.clearance_mm;
        compression = k.compression_ratio.unwrap_or_else(|| 1.0 + cc(bore_mm, stroke_mm) / block_vc);
        piston_kg = STOCK_PISTON_KG * (bore_mm / 86.0).powi(2);
        piston_mat = piston_material(k.alloy);
        pin_mm = k.pin_mm.unwrap_or(22.0);
    } else {
        bore_mm = piston.bore_mm;
        ch_mm = piston.compression_height_mm;
        pin_mm = piston.pin_dia_mm;
        let vc_design = cc(bore_mm, piston.for_stroke_mm) / (piston.compression_ratio - 1.0);
        let design_clear = piston.for_deck_mm - (piston.for_stroke_mm / 2.0 + piston.for_rod_length_mm + ch_mm);
        let clear = crank.deck_mm - (stroke_mm / 2.0 + rod_mm + ch_mm);
        let vc = vc_design + cc(bore_mm, clear - design_clear);
        compression = 1.0 + cc(bore_mm, stroke_mm) / vc;
        piston_kg = piston.weight_g.map(|g| g / 1000.0).unwrap_or(STOCK_PISTON_KG * (bore_mm / 86.0).powi(2));
        if piston.id != "stock" && piston.weight_g.is_none() {
            estimated.insert("pistonWeight".into(), "not listed; stock weight scaled by bore area".into());
        }
        piston_mat = piston.material;
    }
    let requires_sleeves = bore_mm > block.bore_mm + MAX_OVERBORE_MM + 1e-9;
    let deck_clearance_mm = crank.deck_mm - (stroke_mm / 2.0 + rod_mm + ch_mm);
    let rod_recip_kg = rod.weight_g.map(|g| g / 1000.0 * ROD_RECIP_SHARE).unwrap_or(STOCK_ROD_RECIP_KG);
    let housing_mm = if rod.id == "stock" { crank.housing_mm.unwrap_or(51.0) } else { rod.big_end_mm };
    let parts_stock = crank.id == "stock" && piston.id == "stock" && rod.id == "stock";
    let geom_stock = parts_stock && block.id == "K20Z3" && head.id == "K20Z3";
    if requires_sleeves {
        issues.push(format!(
            "{} mm needs the block re-sleeved. Picking these pistons fits ductile iron sleeves; set stock sleeves under Materials to see them bored {:.2} mm thinner per side.",
            fmt_num(bore_mm), (bore_mm - block.bore_mm) / 2.0
        ));
    }
    let piston_name = crank.kit_piston.as_ref().map(|k| k.name.clone()).unwrap_or(piston.name.clone());
    let rod_name = crank.kit_rod.clone().unwrap_or(rod.name.clone());
    let materials = FittedMaterials {
        piston: piston_mat,
        rod: if crank.kit_rod.is_some() { rod_material("chromoly") } else { rod.material_id },
        crank: crank.material,
    };
    BottomEnd {
        stock: geom_stock && setup.pump.id == "rbc", geom_stock, block, head, pump: setup.pump, head_fit, pump_fit: setup.pump_fit,
        deck_mm: crank.deck_mm, crank, piston, rod, issues, estimated, bore_mm, stroke_mm, rod_mm, compression_height_mm: ch_mm, deck_clearance_mm,
        compression, displacement_l: if geom_stock { 1.998 } else { 4.0 * cc(bore_mm, stroke_mm) / 1000.0 },
        piston_kg, rod_recip_kg, pin_mm, big_end_bearing_mm: housing_mm - 3.0, requires_sleeves, piston_name, rod_name, materials,
    }
}
