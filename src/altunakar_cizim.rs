//! Altunakar: bağlıyken ekran çerçevesi ve beyaz tahta (uzak ekrana çizim).
//!
//! Platformdan bağımsız kısım: şekil deposu ve şekillerin boyanacak çizgilere çevrilmesi.
//! Karşı taraftaki katman (Windows'ta tiny-skia, Mac'te piet) yalnız bu çizgileri boyar;
//! böylece şekil mantığı tek yerde ve birim testli.
//! Koordinatlar fare olaylarıyla aynıdır (uzak ekran koordinatı); katman kendi pencere
//! başlangıcını `kaydir` ile çıkarır.

use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const ARAC_KALEM: u32 = 1;
pub const ARAC_CIZGI: u32 = 2;
pub const ARAC_OK: u32 = 3;
pub const ARAC_DIKDORTGEN: u32 = 4;
pub const ARAC_ELIPS: u32 = 5;

/// Bir şekildeki en çok nokta (kalemle uzun karalama sınırsız büyümesin).
pub const EN_COK_NOKTA: usize = 4000;
/// Bir bağlantıdaki en çok şekil; aşılınca en eskisi silinir.
pub const EN_COK_SEKIL: usize = 300;
pub const EN_INCE: f32 = 1.0;
pub const EN_KALIN: f32 = 40.0;

/// Çerçeve rengi (Altunakar turuncusu, ARGB) ve kalınlığı (piksel / Mac'te nokta).
pub const CERCEVE_ARGB: u32 = 0xFFFF_8C1A;
pub const CERCEVE_KALINLIK: f32 = 5.0;

/// Karşı tarafın ayarı: "N" ise bağlıyken ekran çerçevesi gösterilmez (varsayılan açık).
pub const SECENEK_CERCEVE: &str = "altunakar-ekran-cercevesi";

/// Elipsin kaç doğru parçasıyla çizileceği.
const ELIPS_PARCA: usize = 72;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Cizim {
    /// Şeklin bağlantı içindeki kimliği.
    pub kimlik: u32,
    pub arac: u32,
    pub argb: u32,
    pub kalinlik: f32,
    pub noktalar: Vec<(f32, f32)>,
    /// true: kalem şeklinin mevcut noktalarına ekle; false: şeklin noktalarını değiştir.
    pub ekle: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Cerceve {
    pub etiket: String,
}

#[inline]
pub fn anahtar_cerceve(conn_id: i32) -> String {
    format!("{}-cerceve", conn_id)
}

#[inline]
pub fn anahtar_cizim(conn_id: i32) -> String {
    format!("{}-cizim", conn_id)
}

/// Çerçevenin üstündeki etiket. Doğrulanmış teknisyen adı ya da bağlananın adı eklenir.
pub fn cerceve_etiketi(ad: &str) -> String {
    cerceve_etiketi_onekli("Altunakar Destek bağlı", ad)
}

/// `onek`: program diline çevrilmiş "Altunakar Destek bağlı".
pub fn cerceve_etiketi_onekli(onek: &str, ad: &str) -> String {
    let ad: String = ad.trim().chars().take(40).collect();
    if ad.is_empty() {
        onek.to_owned()
    } else {
        format!("{} · {}", onek, ad)
    }
}

/// Protokoldeki düz diziyi (x0,y0,x1,y1,...) nokta çiftlerine çevirir; tek kalan atılır.
pub fn ciftlere(duz: &[i32]) -> Vec<(f32, f32)> {
    duz.chunks_exact(2)
        .take(EN_COK_NOKTA)
        .map(|p| (p[0] as f32, p[1] as f32))
        .collect()
}

/// Arayüzden gelen "x,y,x,y" metnini düz diziye çevirir. Bozuk sayı ya da tek sayıda
/// eleman varsa None.
pub fn metinden_noktalar(s: &str) -> Option<Vec<i32>> {
    let s = s.trim();
    if s.is_empty() {
        return Some(Vec::new());
    }
    let v: Vec<i32> = s
        .split(',')
        .map(|p| p.trim().parse::<i32>().ok())
        .collect::<Option<Vec<_>>>()?;
    if v.len() % 2 != 0 {
        return None;
    }
    Some(v)
}

fn gecerli_arac(arac: u32) -> bool {
    (ARAC_KALEM..=ARAC_ELIPS).contains(&arac)
}

/// Bağlantı (anahtar) başına şekiller.
#[derive(Debug, Default)]
pub struct SekilDeposu {
    sekiller: HashMap<String, BTreeMap<u32, Cizim>>,
}

impl SekilDeposu {
    pub fn uygula(&mut self, anahtar: &str, mut c: Cizim) {
        if !gecerli_arac(c.arac) {
            return;
        }
        c.kalinlik = if c.kalinlik.is_finite() {
            c.kalinlik.clamp(EN_INCE, EN_KALIN)
        } else {
            EN_INCE
        };
        let harita = self.sekiller.entry(anahtar.to_owned()).or_default();
        if c.ekle && c.arac == ARAC_KALEM {
            if let Some(s) = harita.get_mut(&c.kimlik) {
                let kalan = EN_COK_NOKTA.saturating_sub(s.noktalar.len());
                s.noktalar.extend(c.noktalar.into_iter().take(kalan));
                return;
            }
        }
        c.noktalar.truncate(EN_COK_NOKTA);
        if !harita.contains_key(&c.kimlik) && harita.len() >= EN_COK_SEKIL {
            if let Some(&en_eski) = harita.keys().next() {
                harita.remove(&en_eski);
            }
        }
        harita.insert(c.kimlik, c);
    }

    pub fn temizle(&mut self, anahtar: &str) {
        self.sekiller.remove(anahtar);
    }

    pub fn bos_mu(&self) -> bool {
        self.sekiller.values().all(|h| h.is_empty())
    }

    /// Bütün şekiller, çizim sırasıyla (bağlantı içinde kimlik sırası).
    pub fn hepsi(&self) -> impl Iterator<Item = &Cizim> {
        self.sekiller.values().flat_map(|h| h.values())
    }
}

/// Okun ucu: bitiş noktasından geriye ±28°, boyu kalınlıkla büyür. (kanat1, uç, kanat2)
pub fn ok_ucu(p0: (f32, f32), p1: (f32, f32), kalinlik: f32) -> Option<[(f32, f32); 3]> {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let boy = (dx * dx + dy * dy).sqrt();
    if boy < 1.0 {
        return None;
    }
    let kanat = (4.0 * kalinlik + 12.0).min(boy * 0.6).max(6.0);
    let aci = dy.atan2(dx);
    let yay = 28f32.to_radians();
    let k1 = (
        p1.0 - kanat * (aci - yay).cos(),
        p1.1 - kanat * (aci - yay).sin(),
    );
    let k2 = (
        p1.0 - kanat * (aci + yay).cos(),
        p1.1 - kanat * (aci + yay).sin(),
    );
    Some([k1, p1, k2])
}

fn kaydirilmis(p: (f32, f32), kaydir: (f32, f32)) -> (f32, f32) {
    (p.0 - kaydir.0, p.1 - kaydir.1)
}

/// Şekli boyanacak çizgilere (açık ya da kapalı nokta dizileri) çevirir.
pub fn sekil_cizgileri(c: &Cizim, kaydir: (f32, f32)) -> Vec<Vec<(f32, f32)>> {
    let n: Vec<(f32, f32)> = c.noktalar.iter().map(|&p| kaydirilmis(p, kaydir)).collect();
    if n.is_empty() {
        return Vec::new();
    }
    let ilk = n[0];
    let son = *n.last().unwrap();
    match c.arac {
        ARAC_KALEM => {
            if n.len() == 1 {
                // Tek tık: görünür kısa bir nokta.
                vec![vec![ilk, (ilk.0 + 0.5, ilk.1 + 0.5)]]
            } else {
                vec![n]
            }
        }
        ARAC_CIZGI => vec![vec![ilk, son]],
        ARAC_OK => {
            let mut v = vec![vec![ilk, son]];
            if let Some(uc) = ok_ucu(ilk, son, c.kalinlik) {
                v.push(uc.to_vec());
            }
            v
        }
        ARAC_DIKDORTGEN => {
            let (x0, y0, x1, y1) = (ilk.0.min(son.0), ilk.1.min(son.1), ilk.0.max(son.0), ilk.1.max(son.1));
            vec![vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]]
        }
        ARAC_ELIPS => {
            let (cx, cy) = ((ilk.0 + son.0) / 2.0, (ilk.1 + son.1) / 2.0);
            let (rx, ry) = (((son.0 - ilk.0) / 2.0).abs(), ((son.1 - ilk.1) / 2.0).abs());
            let v: Vec<(f32, f32)> = (0..=ELIPS_PARCA)
                .map(|i| {
                    let t = i as f32 / ELIPS_PARCA as f32 * std::f32::consts::TAU;
                    (cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect();
            vec![v]
        }
        _ => Vec::new(),
    }
}

/// Her ekranın kenarına, kalınlığın yarısı kadar içeriden kapalı dikdörtgen.
/// `ekranlar`: (x, y, genişlik, yükseklik), pencere koordinatında.
pub fn cerceve_cizgileri(ekranlar: &[(f32, f32, f32, f32)], kalinlik: f32) -> Vec<Vec<(f32, f32)>> {
    let y = kalinlik / 2.0;
    ekranlar
        .iter()
        .filter(|e| e.2 > kalinlik && e.3 > kalinlik)
        .map(|&(x, ust, w, h)| {
            let (x0, y0, x1, y1) = (x + y, ust + y, x + w - y, ust + h - y);
            vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
        })
        .collect()
}

/// ARGB → (r, g, b, a).
#[inline]
pub fn argb_parcala(argb: u32) -> (u8, u8, u8, u8) {
    (
        (argb >> 16 & 0xFF) as u8,
        (argb >> 8 & 0xFF) as u8,
        (argb & 0xFF) as u8,
        (argb >> 24 & 0xFF) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cizim(kimlik: u32, arac: u32, noktalar: &[(f32, f32)], ekle: bool) -> Cizim {
        Cizim {
            kimlik,
            arac,
            argb: 0xFFFF0000,
            kalinlik: 4.0,
            noktalar: noktalar.to_vec(),
            ekle,
        }
    }

    #[test]
    fn noktalar_metinden_ve_protokolden() {
        assert_eq!(metinden_noktalar("10, 20,30,-40"), Some(vec![10, 20, 30, -40]));
        assert_eq!(metinden_noktalar(""), Some(vec![]));
        assert_eq!(metinden_noktalar("1,2,3"), None, "tek sayıda eleman");
        assert_eq!(metinden_noktalar("1,x"), None);
        assert_eq!(ciftlere(&[1, 2, 3, 4, 5]), vec![(1.0, 2.0), (3.0, 4.0)], "tek kalan atılır");
        let cok: Vec<i32> = (0..(EN_COK_NOKTA as i32 + 10) * 2).collect();
        assert_eq!(ciftlere(&cok).len(), EN_COK_NOKTA);
    }

    #[test]
    fn kalem_ekler_diger_araclar_degistirir() {
        let mut d = SekilDeposu::default();
        d.uygula("1-cizim", cizim(1, ARAC_KALEM, &[(0.0, 0.0)], false));
        d.uygula("1-cizim", cizim(1, ARAC_KALEM, &[(1.0, 1.0), (2.0, 2.0)], true));
        assert_eq!(d.hepsi().next().unwrap().noktalar.len(), 3, "kalem noktaları birikir");
        d.uygula("1-cizim", cizim(2, ARAC_DIKDORTGEN, &[(0.0, 0.0), (5.0, 5.0)], false));
        d.uygula("1-cizim", cizim(2, ARAC_DIKDORTGEN, &[(0.0, 0.0), (9.0, 9.0)], false));
        let r = d.hepsi().find(|c| c.kimlik == 2).unwrap();
        assert_eq!(r.noktalar, vec![(0.0, 0.0), (9.0, 9.0)], "sürüklerken şekil güncellenir");
        // Bilinmeyen araç yok sayılır, kalınlık sınırlanır.
        d.uygula("1-cizim", cizim(3, 99, &[(0.0, 0.0)], false));
        assert!(d.hepsi().all(|c| c.kimlik != 3));
        let mut kalin = cizim(4, ARAC_CIZGI, &[(0.0, 0.0), (1.0, 1.0)], false);
        kalin.kalinlik = 500.0;
        d.uygula("1-cizim", kalin);
        assert_eq!(d.hepsi().find(|c| c.kimlik == 4).unwrap().kalinlik, EN_KALIN);
        // Temizle yalnız o bağlantının çizimlerini siler.
        d.uygula("2-cizim", cizim(1, ARAC_CIZGI, &[(0.0, 0.0), (1.0, 1.0)], false));
        d.temizle("1-cizim");
        assert_eq!(d.hepsi().count(), 1);
        d.temizle("2-cizim");
        assert!(d.bos_mu());
    }

    #[test]
    fn sinirlar_asilmaz() {
        let mut d = SekilDeposu::default();
        for i in 0..(EN_COK_SEKIL as u32 + 5) {
            d.uygula("k", cizim(i, ARAC_CIZGI, &[(0.0, 0.0), (1.0, 1.0)], false));
        }
        assert_eq!(d.hepsi().count(), EN_COK_SEKIL);
        assert!(d.hepsi().all(|c| c.kimlik >= 5), "en eskiler silinir");
        let mut d = SekilDeposu::default();
        d.uygula("k", cizim(1, ARAC_KALEM, &[(0.0, 0.0)], false));
        let cok: Vec<(f32, f32)> = (0..EN_COK_NOKTA + 50).map(|i| (i as f32, 0.0)).collect();
        d.uygula("k", cizim(1, ARAC_KALEM, &cok, true));
        assert_eq!(d.hepsi().next().unwrap().noktalar.len(), EN_COK_NOKTA);
    }

    #[test]
    fn sekiller_cizgiye_doner() {
        let k = (100.0, 50.0);
        let r = sekil_cizgileri(&cizim(1, ARAC_DIKDORTGEN, &[(110.0, 90.0), (104.0, 60.0)], false), k);
        assert_eq!(r, vec![vec![(4.0, 10.0), (10.0, 10.0), (10.0, 40.0), (4.0, 40.0), (4.0, 10.0)]], "köşeler sıralanır, kaydırılır");
        let ok = sekil_cizgileri(&cizim(1, ARAC_OK, &[(0.0, 0.0), (100.0, 0.0)], false), (0.0, 0.0));
        assert_eq!(ok.len(), 2, "gövde + uç");
        let uc = &ok[1];
        assert_eq!(uc[1], (100.0, 0.0), "uç bitiş noktasında");
        assert!(uc[0].0 < 100.0 && uc[2].0 < 100.0 && uc[0].1 * uc[2].1 < 0.0, "kanatlar geride ve iki yanda");
        assert!(ok_ucu((5.0, 5.0), (5.0, 5.0), 4.0).is_none());
        let e = sekil_cizgileri(&cizim(1, ARAC_ELIPS, &[(0.0, 0.0), (20.0, 10.0)], false), (0.0, 0.0));
        let e = &e[0];
        assert_eq!(e.len(), ELIPS_PARCA + 1);
        assert!((e[0].0 - 20.0).abs() < 1e-3 && (e[0].1 - 5.0).abs() < 1e-3, "sağ uç");
        assert!(e.iter().all(|p| p.0 >= -1e-3 && p.0 <= 20.001 && p.1 >= -1e-3 && p.1 <= 10.001), "kutunun içinde");
        let tek = sekil_cizgileri(&cizim(1, ARAC_KALEM, &[(3.0, 3.0)], false), (0.0, 0.0));
        assert_eq!(tek[0].len(), 2, "tek tık görünür nokta");
        assert!(sekil_cizgileri(&cizim(1, ARAC_CIZGI, &[], false), (0.0, 0.0)).is_empty());
    }

    #[test]
    fn cerceve_her_ekranin_icinden() {
        let c = cerceve_cizgileri(&[(0.0, 0.0, 1920.0, 1080.0), (1920.0, 0.0, 1280.0, 1024.0), (0.0, 0.0, 3.0, 3.0)], 6.0);
        assert_eq!(c.len(), 2, "çok küçük ekran atlanır");
        assert_eq!(c[0][0], (3.0, 3.0));
        assert_eq!(c[0][2], (1917.0, 1077.0));
        assert_eq!(c[1][0], (1923.0, 3.0));
        assert_eq!(cerceve_etiketi("  Tuna "), "Altunakar Destek bağlı · Tuna");
        assert_eq!(cerceve_etiketi(""), "Altunakar Destek bağlı");
        assert_eq!(cerceve_etiketi_onekli("Altunakar Support verbunden", "Tuna"), "Altunakar Support verbunden · Tuna");
        assert_eq!(anahtar_cerceve(7), "7-cerceve");
        assert_eq!(anahtar_cizim(7), "7-cizim");
        assert_eq!(argb_parcala(0x80FF8C1A), (0xFF, 0x8C, 0x1A, 0x80));
    }
}
