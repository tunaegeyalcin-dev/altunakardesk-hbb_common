//! Altunakar: sohbet geçmişi kaydı.
//!
//! Her bağlantının sohbeti günlük bir metin dosyasına eklenir:
//! `<Belgeler>/AltunakarDesk/Sohbetler/<YYYY-MM-DD>_<numara>.txt`, satır başına
//! `[YYYY-MM-DD SS:DD:ss] Kim: metin`. Bağlanan (teknisyen) tarafta varsayılan açık,
//! bağlanılan (müşteri) tarafta varsayılan kapalı: müşterinin bilgisayarına habersiz kayıt düşmesin.

use crate::ResultType;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Bağlanan taraf (yerel ayar). "N" değilse açık.
pub const SECENEK_GIDEN: &str = "altunakar-sohbet-kaydi";
/// Bağlanılan taraf (yerel ayar). Yalnız "Y" ise açık; "allow-" öneki arayüzde de varsayılanı kapalı yapar.
pub const SECENEK_GELEN: &str = "allow-altunakar-sohbet-kaydi-gelen";

#[inline]
pub fn giden_acik(deger: &str) -> bool {
    deger != "N"
}

#[inline]
pub fn gelen_acik(deger: &str) -> bool {
    deger == "Y"
}

/// `<tarih>_<numara>.txt`; numaradaki dosya adına uygun olmayan karakterler atılır.
pub fn dosya_adi(tarih: &str, numara: &str) -> String {
    let n: String = numara
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(40)
        .collect();
    let n = if n.is_empty() { "bilinmeyen".to_owned() } else { n };
    format!("{}_{}.txt", tarih, n)
}

/// Tek kayıt satırı. Çok satırlı mesajın sonraki satırları girintili yazılır.
pub fn satir(zaman: &str, kim: &str, metin: &str) -> String {
    let metin = metin.replace("\r\n", "\n").replace('\r', "\n");
    let metin = metin.trim_end_matches('\n');
    let govde = metin.replace('\n', "\n    ");
    let kim = kim.replace(['\n', '\r'], " ");
    format!("[{}] {}: {}\n", zaman, kim.trim(), govde)
}

/// Satırı `klasor` içindeki günlük dosyaya ekler (klasör yoksa oluşturur).
pub fn ekle(
    klasor: &Path,
    tarih: &str,
    zaman: &str,
    numara: &str,
    kim: &str,
    metin: &str,
) -> ResultType<PathBuf> {
    std::fs::create_dir_all(klasor)?;
    let yol = klasor.join(dosya_adi(tarih, numara));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&yol)?;
    f.write_all(satir(zaman, kim, metin).as_bytes())?;
    Ok(yol)
}

/// `<Belgeler>/AltunakarDesk/Sohbetler` (Belgeler bulunamazsa ev klasörü).
pub fn varsayilan_klasor() -> PathBuf {
    let kok = directories_next::UserDirs::new()
        .and_then(|u| u.document_dir().map(|p| p.to_path_buf()))
        .or_else(|| directories_next::BaseDirs::new().map(|b| b.home_dir().to_path_buf()))
        .unwrap_or_else(std::env::temp_dir);
    kok.join("AltunakarDesk").join("Sohbetler")
}

/// Şimdiki zamanla varsayılan klasöre yazar; hata yalnız günlüğe düşer (sohbeti bozmaz).
pub fn kaydet(numara: &str, kim: &str, metin: &str) {
    let simdi = chrono::Local::now();
    let tarih = simdi.format("%Y-%m-%d").to_string();
    let zaman = simdi.format("%Y-%m-%d %H:%M:%S").to_string();
    if let Err(e) = ekle(&varsayilan_klasor(), &tarih, &zaman, numara, kim, metin) {
        log::warn!("Sohbet kaydi yazilamadi: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varsayilanlar() {
        assert!(giden_acik(""), "teknisyen tarafı varsayılan açık");
        assert!(!giden_acik("N"));
        assert!(!gelen_acik(""), "müşteri tarafı varsayılan kapalı");
        assert!(gelen_acik("Y"));
        assert!(SECENEK_GELEN.starts_with("allow-"), "arayüzde varsayılan kapalı görünsün");
    }

    #[test]
    fn dosya_adi_ve_satir() {
        assert_eq!(dosya_adi("2026-10-22", "407503251"), "2026-10-22_407503251.txt");
        assert_eq!(dosya_adi("2026-10-22", "../../x y"), "2026-10-22_xy.txt", "yol kaçışı yok");
        assert_eq!(dosya_adi("2026-10-22", "/"), "2026-10-22_bilinmeyen.txt");
        assert_eq!(satir("2026-10-22 10:15:03", "Siz", "merhaba"), "[2026-10-22 10:15:03] Siz: merhaba\n");
        assert_eq!(
            satir("z", "Tuna\n", "bir\r\niki\n"),
            "[z] Tuna: bir\n    iki\n",
            "çok satır girintili, sondaki satır sonu atılır"
        );
    }

    #[test]
    fn dosyaya_eklenir() {
        let k = std::env::temp_dir().join(format!("altunakar-sohbet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&k);
        let y = ekle(&k.join("Sohbetler"), "2026-10-22", "2026-10-22 10:00:00", "123", "Siz", "a").unwrap();
        ekle(&k.join("Sohbetler"), "2026-10-22", "2026-10-22 10:00:05", "123", "Karşı taraf", "b").unwrap();
        let icerik = std::fs::read_to_string(&y).unwrap();
        assert_eq!(icerik, "[2026-10-22 10:00:00] Siz: a\n[2026-10-22 10:00:05] Karşı taraf: b\n");
        std::fs::remove_dir_all(&k).ok();
    }
}
