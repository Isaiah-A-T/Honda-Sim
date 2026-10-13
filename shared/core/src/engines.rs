//! Which K-series block, head and oil pump the engine is built from, and whether they go together
//! (web/src/engines.js). The data is docs/engines.json, built from docs/k-series-specs.json.

use crate::gasflow::Lobes;
use crate::tune::Tune;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    pub hp: f64,
    pub hp_rpm: f64,
    pub torque_lbft: f64,
    pub torque_rpm: f64,
    pub redline_rpm: f64,
    pub rev_limit_rpm: Option<f64>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: String,
    pub name: String,
    pub family: String,
    pub series: String,
    pub vehicles: Vec<String>,
    pub bore_mm: f64,
    pub stroke_mm: f64,
    pub rod_mm: f64,
    pub deck_mm: f64,
    pub compression: f64,
    pub compression_height_mm: f64,
    pub pin_mm: f64,
    pub big_end_mm: f64,
    pub balance_shafts: bool,
    pub oil_pump: String,
    pub head: String,
    pub rating: Rating,
    pub aftermarket_bottom_end: bool,
    pub pistons_vs_k20_head: Option<String>,
    pub estimated: BTreeMap<String, String>,
    pub sources: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeadLobes {
    pub in_lift: f64,
    pub in_dur: f64,
    pub ex_lift: f64,
    pub ex_dur: f64,
    pub in_valves: Option<f64>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct HeadCams {
    pub low: HeadLobes,
    pub high: HeadLobes,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    pub id: String,
    pub name: String,
    pub family: String,
    /// "dohc", "economy" (intake VTEC, one intake valve cracked open on the low cam) or "intake".
    pub vtec: String,
    pub vtec_rpm: f64,
    pub in_valve_mm: f64,
    pub ex_valve_mm: f64,
    pub part_number: Option<String>,
    pub cams: HeadCams,
    pub estimated: BTreeMap<String, String>,
    pub sources: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct PumpFitRule {
    pub blocks: Vec<String>,
    pub needs: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OilPump {
    pub id: String,
    pub name: String,
    pub part_number: String,
    pub balance_shafts: bool,
    pub flow_scale: f64,
    pub max_rpm: f64,
    pub fits: Vec<PumpFitRule>,
    pub estimated: BTreeMap<String, String>,
    pub sources: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CompatRule {
    pub heads: Vec<String>,
    pub blocks: Vec<String>,
    pub works: String,
    pub needs: Vec<String>,
    pub note: String,
    pub sources: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Engines {
    pub blocks: Vec<Block>,
    pub heads: Vec<Head>,
    pub oil_pumps: Vec<OilPump>,
    pub compatibility: Vec<CompatRule>,
}

pub fn engines() -> &'static Engines {
    static E: OnceLock<Engines> = OnceLock::new();
    E.get_or_init(|| serde_json::from_str(include_str!("../data/engines.json")).expect("engines.json"))
}
pub fn block(id: &str) -> Option<&'static Block> {
    engines().blocks.iter().find(|b| b.id == id)
}
pub fn head(id: &str) -> Option<&'static Head> {
    engines().heads.iter().find(|h| h.id == id)
}
pub fn oil_pump(id: &str) -> Option<&'static OilPump> {
    engines().oil_pumps.iter().find(|p| p.id == id)
}

pub const STOCK_BLOCK: &str = "K20Z3";
pub const STOCK_HEAD: &str = "K20Z3";
pub const STOCK_PUMP: &str = "rbc";

/// "yes", "mods" or "no", what the swap needs, and a note.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Fit {
    pub works: String,
    pub needs: Vec<String>,
    pub note: String,
    pub sources: Vec<String>,
}

pub fn head_fit(head: &Head, block: &Block) -> Fit {
    if let Some(r) = engines().compatibility.iter().find(|r| r.heads.contains(&head.id) && r.blocks.contains(&block.id)) {
        return Fit { works: r.works.clone(), needs: r.needs.clone(), note: r.note.clone(), sources: r.sources.clone() };
    }
    if head.family == block.family {
        let note = if head.id == block.head {
            "This block's own head.".to_string()
        } else {
            format!("Same {} family, so it bolts on (inferred; not separately documented).", head.family)
        };
        return Fit { works: "yes".into(), needs: vec![], note, sources: vec![] };
    }
    let econ = if head.vtec == "economy" { "economy " } else { "" };
    Fit { works: "no".into(), needs: vec![], note: format!("No documented build puts a {} {}head on a {} block.", head.family, econ, block.family), sources: vec![] }
}

pub fn pump_fit(pump: &OilPump, block: &Block) -> Fit {
    match pump.fits.iter().find(|f| f.blocks.contains(&block.id)) {
        None => Fit { works: "no".into(), needs: vec![], note: format!("Not listed for the {}.", block.id), sources: vec![] },
        Some(r) => Fit { works: if r.needs.is_empty() { "yes" } else { "mods" }.into(), needs: r.needs.clone(), note: String::new(), sources: vec![] },
    }
}

#[derive(Clone, Debug)]
pub struct Setup {
    pub block: &'static Block,
    pub head: &'static Head,
    pub pump: &'static OilPump,
    pub head_fit: Fit,
    pub pump_fit: Fit,
    pub issues: Vec<String>,
    pub stock: bool,
}

/// The block, head and pump actually fitted, after the fit checks.
pub fn engine_setup(tune: &Tune) -> Setup {
    let sel = &tune.engine;
    let mut issues = Vec::new();
    // Saved tunes from before blocks were selectable picked a K24 through the crank list.
    let legacy_k24 = sel.block.is_none() && tune.parts.crank.as_deref() == Some("honda-k24-99-hybrid");
    let want_block = if legacy_k24 { "K24A2" } else { sel.block.as_deref().unwrap_or(STOCK_BLOCK) };
    let blk = block(want_block).unwrap_or_else(|| {
        issues.push(format!("Unknown block \"{want_block}\"; using the K20Z3."));
        block(STOCK_BLOCK).unwrap()
    });
    let want_head = sel.head.clone().unwrap_or_else(|| if legacy_k24 { "K20Z3".into() } else { blk.head.clone() });
    let mut hd = head(&want_head).unwrap_or_else(|| {
        issues.push(format!("Unknown head \"{want_head}\"; using the {}'s own.", blk.id));
        head(&blk.head).unwrap()
    });
    let mut hf = head_fit(hd, blk);
    if hf.works == "no" {
        issues.push(format!("The {} can't go on the {}: {} Using the {}'s own head.", hd.name, blk.id, hf.note, blk.id));
        hd = head(&blk.head).unwrap();
        hf = head_fit(hd, blk);
    }
    let want_pump = sel.oil_pump.clone().unwrap_or_else(|| blk.oil_pump.clone());
    let mut pm = oil_pump(&want_pump).unwrap_or_else(|| {
        issues.push(format!("Unknown oil pump \"{want_pump}\"; using the {}'s own.", blk.id));
        oil_pump(&blk.oil_pump).unwrap()
    });
    let mut pf = pump_fit(pm, blk);
    if pf.works == "no" {
        issues.push(format!("The {} can't go in the {}: {} Using the {}'s own pump.", pm.name, blk.id, pf.note, blk.id));
        pm = oil_pump(&blk.oil_pump).unwrap();
        pf = pump_fit(pm, blk);
    }
    let stock = blk.id == STOCK_BLOCK && hd.id == STOCK_HEAD && pm.id == STOCK_PUMP;
    Setup { block: blk, head: hd, pump: pm, head_fit: hf, pump_fit: pf, issues, stock }
}

pub fn head_lobes(l: &HeadLobes) -> Lobes {
    Lobes { in_lift: l.in_lift, in_dur: l.in_dur, ex_lift: l.ex_lift, ex_dur: l.ex_dur, in_valves: l.in_valves }
}

/// Balance shaft drag (Nm), web/src/engines.js: 5.6 kW at 7,800 rpm, in proportion to speed.
pub const BALANCER_KW_AT_7800: f64 = 5.6;
pub fn balancer_nm(rpm: f64) -> f64 {
    BALANCER_KW_AT_7800 * 1000.0 / (7800.0 * std::f64::consts::PI / 30.0) * rpm.max(0.0) / 7800.0
}
