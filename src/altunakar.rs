//! Altunakar: ekip sunucusunun "Altunakar teknisyeni (dogrulandi)" bileti (Asama 2).
//! Bilet = base64(Ed25519 imza || JSON yuk), libsodium crypto_sign birlesik bicimi;
//! ekip sunucusu (Bina deposu altunakardesk/ekip/bilet.go) imzalar, kontrol edilen
//! cihaz bu dosyadaki gomulu ACIK anahtarla cevrimdisi dogrular.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use sodiumoxide::crypto::sign;

/// Bileti imzalayan ekip sunucusu anahtari (`altunakardesk-ekip bilet-anahtari`).
pub const EKIP_BILET_PK: &str = "HUo9A5k62Qwx33O0PmE/KA0HYzrC5B9ERJkry/XC2RQ=";
const BILET_TURU: &str = "altunakardesk-bilet-v1";

#[derive(serde_derive::Deserialize)]
struct BiletYuku {
    t: String,
    ad: String,
    k: String,
    h: String,
    s: i64,
}

/// Bileti dogrular; gecerliyse teknisyenin adini doner. Gecersiz, suresi dolmus,
/// baska cihaza ya da baska kaynaga kesilmis bilet `None`.
/// `kaynak` baglananin bildirdigi numara (baska merkezden "id@sunucu" olabilir),
/// `hedef` bu cihazin numarasi, `simdi` unix saniye.
pub fn bilet_dogrula(bilet: &str, acik_anahtar: &str, kaynak: &str, hedef: &str, simdi: i64) -> Option<String> {
    if bilet.is_empty() || bilet.len() > 4096 {
        return None;
    }
    let _ = sodiumoxide::init();
    let pk = sign::PublicKey::from_slice(&STANDARD.decode(acik_anahtar).ok()?)?;
    let yuk = sign::verify(&STANDARD.decode(bilet).ok()?, &pk).ok()?;
    let y: BiletYuku = serde_json::from_slice(&yuk).ok()?;
    let kaynak = kaynak.split('@').next().unwrap_or(kaynak);
    if y.t != BILET_TURU || y.k != kaynak || y.h != hedef || simdi > y.s {
        return None;
    }
    let ad: String = y.ad.chars().filter(|c| !c.is_control()).take(60).collect();
    let ad = ad.trim().to_owned();
    if ad.is_empty() {
        None
    } else {
        Some(ad)
    }
}

/// Bilet isteginin govdesi (POST /api/altunakar/bilet).
pub fn bilet_istegi(kaynak: &str, hedef: &str) -> String {
    serde_json::json!({ "kaynak": kaynak, "hedef": hedef }).to_string()
}

/// Ekip sunucusunun yanitindan bileti cikarir; bicimsiz yanitta `None`.
pub fn bilet_yanitindan(yanit: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(yanit).ok()?;
    v.get("bilet")?.as_str().map(|s| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ekip sunucusunun TestBiletImzaVeYuk testindeki SABIT ornek (ayni tohum, ayni an).
    const ORNEK_PK: &str = "tnTEf61LpD1nhj94YpPovN/Pp8Tb1+Hlu/2+HiUb/Bk=";
    const ORNEK_BILET: &str = "0uoxqc68nXytzRVDy9XLQzitovL0//HZYGEatzVUTux1M0hS89KbuD/lCp1VkPXKEAWnnR2vEZ0kL7uX1XHUDXsidCI6ImFsdHVuYWthcmRlc2stYmlsZXQtdjEiLCJhZCI6IlR1bmEgWWFsw6fEsW4iLCJrIjoiNDA3NTAzMjUxIiwiaCI6IjY2MDk1Njc2IiwicyI6MTc2MDk1NDQwMH0=";
    const SON: i64 = 1760954400;

    #[test]
    fn gecerli_bilet_teknisyen_adini_verir() {
        assert_eq!(
            bilet_dogrula(ORNEK_BILET, ORNEK_PK, "407503251", "66095676", SON - 60).as_deref(),
            Some("Tuna Yalçın")
        );
        assert_eq!(
            bilet_dogrula(ORNEK_BILET, ORNEK_PK, "407503251@desk.tunayalcin.blog", "66095676", SON).as_deref(),
            Some("Tuna Yalçın")
        );
    }

    #[test]
    fn gecersiz_biletler_reddedilir() {
        assert_eq!(bilet_dogrula(ORNEK_BILET, ORNEK_PK, "407503251", "11111111", SON - 60), None, "baska hedef");
        assert_eq!(bilet_dogrula(ORNEK_BILET, ORNEK_PK, "999999999", "66095676", SON - 60), None, "baska kaynak");
        assert_eq!(bilet_dogrula(ORNEK_BILET, ORNEK_PK, "407503251", "66095676", SON + 1), None, "suresi dolmus");
        assert_eq!(bilet_dogrula("", ORNEK_PK, "407503251", "66095676", SON - 60), None, "bos");
        let mut b = STANDARD.decode(ORNEK_BILET).unwrap();
        let n = b.len();
        b[n - 3] ^= 1;
        assert_eq!(bilet_dogrula(&STANDARD.encode(b), ORNEK_PK, "407503251", "66095676", SON - 60), None, "bozuk imza");
        let baska = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        assert_eq!(bilet_dogrula(ORNEK_BILET, baska, "407503251", "66095676", SON - 60), None, "baska anahtar");
    }

    #[test]
    fn gomulu_anahtar_32_bayt() {
        assert_eq!(
            STANDARD.decode(EKIP_BILET_PK).map(|b| b.len()).ok(),
            Some(32),
            "EKIP_BILET_PK, TUNA'daki `altunakardesk-ekip bilet-anahtari` ciktisi olmali"
        );
    }

    #[test]
    fn istek_ve_yanit_bicimi() {
        let g: serde_json::Value = serde_json::from_str(&bilet_istegi("407503251", "66095676")).unwrap();
        assert_eq!(g["kaynak"], "407503251");
        assert_eq!(g["hedef"], "66095676");
        assert_eq!(bilet_yanitindan(r#"{"bilet":"QUJD","son":1}"#).as_deref(), Some("QUJD"));
        assert_eq!(bilet_yanitindan(r#"{"error":"Oturum kapandı"}"#), None);
        assert_eq!(bilet_yanitindan("<html>"), None);
    }
}
