//! Altunakar: kendi GitHub surumlerimizden IMZALI guncelleme.
//!
//! Her surumle birlikte iki dosya yayinlanir (altunakar/yayinla.sh):
//!   AltunakarDesk-surum.json       {"etiket":"1.5.0-8","dosyalar":{"AltunakarDesk-Windows.msi":"<sha256>", ...}}
//!   AltunakarDesk-surum.json.imza  json dosyasinin ham baytlarinin Ed25519 imzasi (base64)
//! Ozel anahtar yalniz TUNA'nin anahtar zincirinde ("altunakardesk-guncelleme-anahtar").
//! Program imzayi ve sonra indirdigi kurulum dosyasinin SHA-256'sini dogrulamadan hicbir sey kurmaz.

use crate::{bail, ResultType};
use std::collections::HashMap;

/// Guncelleme imzasinin acik anahtari (Ed25519, ham 32 bayt, base64).
pub const GUNCELLEME_ACIK_ANAHTAR: &str = "lkjbJ+eUO233PPPvfWTpzePqpuqvDlPu4iO3NDN8fa8=";
/// Surumlerin yayinlandigi GitHub deposu.
pub const DEPO: &str = "tunaegeyalcin-dev/altunakardesk";
pub const BILDIRIM_DOSYASI: &str = "AltunakarDesk-surum.json";
pub const IMZA_DOSYASI: &str = "AltunakarDesk-surum.json.imza";

/// Derlemenin surum etiketi (CI'da ALTUNAKAR_SURUM = git etiketi, or. "1.5.0-8").
/// Yerel/deneme derlemelerinde bos: bu durumda otomatik guncelleme yapilmaz.
pub fn surum() -> &'static str {
    etiketten_surum(option_env!("ALTUNAKAR_SURUM").unwrap_or(""))
}

/// Etiketin onekini atar: "intel-1.5.0-7" (gecici yeniden derleme etiketi) -> "1.5.0-7",
/// "nightly" -> "".
fn etiketten_surum(etiket: &str) -> &str {
    etiket
        .find(|c: char| c.is_ascii_digit())
        .map(|i| &etiket[i..])
        .unwrap_or("")
}

/// "1.5.0-8" / "v1.5.0-8" / "1.5.0" -> [1, 5, 0, 8]. Taninmayan bicim -> None.
pub fn surum_sayisi(s: &str) -> Option<[u32; 4]> {
    let s = s.trim().trim_start_matches('v');
    let (ana, ek) = match s.split_once('-') {
        Some((a, e)) => (a, Some(e)),
        None => (s, None),
    };
    let p: Vec<&str> = ana.split('.').collect();
    if p.len() != 3 {
        return None;
    }
    let mut out = [0u32; 4];
    for (i, x) in p.iter().enumerate() {
        out[i] = x.parse().ok()?;
    }
    if let Some(e) = ek {
        out[3] = e.parse().ok()?;
    }
    Some(out)
}

/// Uzaktaki etiket yereldekinden yeni mi? Yerel surum bilinmiyorsa (deneme derlemesi) hayir.
pub fn daha_yeni(uzak: &str, yerel: &str) -> bool {
    match (surum_sayisi(uzak), surum_sayisi(yerel)) {
        (Some(u), Some(y)) => u > y,
        _ => false,
    }
}

#[derive(Debug, Clone, serde_derive::Deserialize, PartialEq)]
pub struct SurumBildirimi {
    pub etiket: String,
    /// dosya adi -> kucuk harf SHA-256 (hex)
    pub dosyalar: HashMap<String, String>,
}

/// Bildirimi imzasiyla dogrular ve cozer. Imza gecmezse hata.
pub fn bildirimi_dogrula(govde: &[u8], imza_b64: &str) -> ResultType<SurumBildirimi> {
    let pk = sodiumoxide::base64::decode(GUNCELLEME_ACIK_ANAHTAR, sodiumoxide::base64::Variant::Original)
        .map_err(|_| crate::anyhow::anyhow!("acik anahtar cozulemedi"))?;
    bildirimi_anahtarla_dogrula(govde, imza_b64, &pk)
}

fn bildirimi_anahtarla_dogrula(govde: &[u8], imza_b64: &str, pk: &[u8]) -> ResultType<SurumBildirimi> {
    use sodiumoxide::crypto::sign;
    let imza = sodiumoxide::base64::decode(imza_b64.trim(), sodiumoxide::base64::Variant::Original)
        .map_err(|_| crate::anyhow::anyhow!("imza base64 degil"))?;
    let Some(pk) = sign::PublicKey::from_slice(pk) else {
        bail!("acik anahtar boyu yanlis");
    };
    let Some(imza) = sign::Signature::from_bytes(&imza).ok() else {
        bail!("imza boyu yanlis");
    };
    if !sign::verify_detached(&imza, govde, &pk) {
        bail!("guncelleme bildiriminin imzasi GECERSIZ");
    }
    let b: SurumBildirimi = serde_json::from_slice(govde)?;
    if surum_sayisi(&b.etiket).is_none() {
        bail!("bildirimdeki etiket taninmiyor: {}", b.etiket);
    }
    Ok(b)
}

/// Dosyanin SHA-256'si (kucuk harf hex).
pub fn dosya_sha256(yol: &std::path::Path) -> ResultType<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(yol)?;
    let mut h = Sha256::new();
    let mut tampon = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut tampon)?;
        if n == 0 {
            break;
        }
        h.update(&tampon[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{:02x}", b)).collect())
}

/// Indirilen dosyanin bildirimdeki parmak iziyle ayni oldugunu dogrular.
pub fn dosyayi_dogrula(b: &SurumBildirimi, dosya_adi: &str, yol: &std::path::Path) -> ResultType<()> {
    let Some(beklenen) = b.dosyalar.get(dosya_adi) else {
        bail!("bildirimde {} yok", dosya_adi);
    };
    let gercek = dosya_sha256(yol)?;
    if !gercek.eq_ignore_ascii_case(beklenen.trim()) {
        bail!("{} parmak izi tutmuyor", dosya_adi);
    }
    Ok(())
}

/// Windows kurulum dosyasinin sabit adi (yayinla.sh her surume bu adlarla yukler).
pub fn windows_dosya_adi(msi: bool) -> &'static str {
    if msi {
        "AltunakarDesk-Windows.msi"
    } else {
        "AltunakarDesk-Windows.exe"
    }
}

pub fn mac_dosya_adi(apple_islemci: bool) -> &'static str {
    if apple_islemci {
        "AltunakarDesk-Mac-Apple.dmg"
    } else {
        "AltunakarDesk-Mac-Intel.dmg"
    }
}

/// Surum etiketinin indirme klasoru: https://github.com/<DEPO>/releases/download/<etiket>
pub fn indirme_kok(etiket: &str) -> String {
    format!("https://github.com/{}/releases/download/{}", DEPO, etiket)
}

/// Surum sayfasi (SOFTWARE_UPDATE_URL bicimi): https://github.com/<DEPO>/releases/tag/<etiket>
pub fn surum_sayfasi(etiket: &str) -> String {
    format!("https://github.com/{}/releases/tag/{}", DEPO, etiket)
}

/// Son surum bildiriminin adresi (GitHub "latest" yonlendirmesi).
pub fn son_bildirim_adresi(dosya: &str) -> String {
    format!("https://github.com/{}/releases/latest/download/{}", DEPO, dosya)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surum_karsilastirma() {
        assert_eq!(surum_sayisi("1.5.0-8"), Some([1, 5, 0, 8]));
        assert_eq!(surum_sayisi("v1.5.0-12"), Some([1, 5, 0, 12]));
        assert_eq!(surum_sayisi("1.5.0"), Some([1, 5, 0, 0]));
        assert_eq!(surum_sayisi("1.5.0-deneme"), None);
        assert_eq!(surum_sayisi("intel-1.5.0-7"), None);
        assert!(daha_yeni("1.5.0-10", "1.5.0-9"), "sayisal karsilastirma, metin degil");
        assert!(daha_yeni("1.6.0-1", "1.5.0-20"));
        assert!(!daha_yeni("1.5.0-7", "1.5.0-7"));
        assert!(!daha_yeni("1.5.0-6", "1.5.0-7"));
        assert!(!daha_yeni("1.5.0-8", ""), "surumu bilinmeyen deneme derlemesi guncellenmez");
        assert_eq!(etiketten_surum("intel-1.5.0-7"), "1.5.0-7");
        assert_eq!(etiketten_surum("v1.5.0-8"), "1.5.0-8");
        assert_eq!(etiketten_surum("nightly"), "");
    }

    #[test]
    fn imzali_bildirim() {
        use sodiumoxide::crypto::sign;
        let _ = sodiumoxide::init();
        let (pk, sk) = sign::gen_keypair();
        let govde = br#"{"etiket":"1.5.0-8","dosyalar":{"AltunakarDesk-Windows.msi":"ab"}}"#;
        let imza = sodiumoxide::base64::encode(sign::sign_detached(govde, &sk).to_bytes(), sodiumoxide::base64::Variant::Original);
        let b = bildirimi_anahtarla_dogrula(govde, &imza, pk.as_ref()).unwrap();
        assert_eq!(b.etiket, "1.5.0-8");
        let degisik = br#"{"etiket":"1.5.0-9","dosyalar":{"AltunakarDesk-Windows.msi":"ab"}}"#;
        assert!(bildirimi_anahtarla_dogrula(degisik, &imza, pk.as_ref()).is_err(), "degistirilmis govde reddedilir");
        let (pk2, _) = sign::gen_keypair();
        assert!(bildirimi_anahtarla_dogrula(govde, &imza, pk2.as_ref()).is_err(), "baska anahtar reddedilir");
        assert!(bildirimi_dogrula(govde, &imza).is_err(), "gomulu anahtar test anahtari degil");
    }

    /// TUNA'daki gercek ozel anahtarla OpenSSL (pkeyutl -rawin) ile atilmis imza:
    /// yayinla.sh'in urettigi imzanin programda gectigini kanitlar.
    #[test]
    fn gercek_anahtarla_openssl_imzasi() {
        let govde = br#"{"etiket":"1.5.0-0","dosyalar":{"AltunakarDesk-Windows.msi":"00"}}"#;
        let imza = "Ulkrg/0MjCt+USb2Ie+HQRkbHPLTSyb8PwsLef/Je+13mT+ESD2VFW0APJZ55an77jAMumrHnOeouqXDGDvdCw==";
        let b = bildirimi_dogrula(govde, imza).unwrap();
        assert_eq!(b.etiket, "1.5.0-0");
        assert!(bildirimi_dogrula(&govde[..govde.len() - 1], imza).is_err());
    }

    #[test]
    fn dosya_parmak_izi() {
        let d = std::env::temp_dir().join(format!("altunakar-sha-{}", std::process::id()));
        std::fs::write(&d, b"altunakar").unwrap();
        let sha = dosya_sha256(&d).unwrap();
        let mut dosyalar = HashMap::new();
        dosyalar.insert("A.msi".to_owned(), sha.to_uppercase());
        let b = SurumBildirimi { etiket: "1.5.0-8".into(), dosyalar };
        assert!(dosyayi_dogrula(&b, "A.msi", &d).is_ok());
        assert!(dosyayi_dogrula(&b, "B.msi", &d).is_err());
        std::fs::write(&d, b"degisti").unwrap();
        assert!(dosyayi_dogrula(&b, "A.msi", &d).is_err());
        std::fs::remove_file(&d).ok();
    }

    #[test]
    fn adresler() {
        assert_eq!(indirme_kok("1.5.0-8"), "https://github.com/tunaegeyalcin-dev/altunakardesk/releases/download/1.5.0-8");
        assert_eq!(surum_sayfasi("1.5.0-8").replace("/tag/", "/download/"), indirme_kok("1.5.0-8"));
        assert_eq!(windows_dosya_adi(true), "AltunakarDesk-Windows.msi");
    }
}
