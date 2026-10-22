//! Altunakar: "Kayıtları Altunakar'a gönder".
//!
//! Müşteri onay verince programın son günlerdeki kayıt dosyaları tek bir sıkıştırılmış metin
//! dosyasında toplanıp ekip sunucusuna gönderilir (`POST <api>/api/altunakar/kayit-gonder`).
//! Şifre ya da ekran görüntüsü içermez; her dosyanın yalnız sonu alınır, toplam boy sınırlıdır.

use crate::ResultType;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Kaç günlük kayıt.
pub const GUN: u64 = 3;
/// Bir dosyadan en çok bu kadarı (sonundan) alınır.
pub const DOSYA_SINIRI: usize = 1024 * 1024;
/// Sıkıştırmadan önceki toplam sınır.
pub const TOPLAM_SINIR: usize = 6 * 1024 * 1024;
/// Ekip sunucusundaki karşılama noktası.
pub const UC: &str = "/api/altunakar/kayit-gonder";

/// `kok` altındaki (alt klasörler dahil) .log dosyalarından son `gun` günde değişmiş olanlar,
/// en yeni önce.
pub fn dosyalari_sec(kok: &Path, simdi: SystemTime, gun: u64) -> Vec<PathBuf> {
    let sinir = simdi
        .checked_sub(Duration::from_secs(gun * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut bulunan: Vec<(SystemTime, PathBuf)> = Vec::new();
    let mut yigin = vec![kok.to_path_buf()];
    let mut gezilen = 0usize;
    while let Some(d) = yigin.pop() {
        gezilen += 1;
        if gezilen > 200 {
            break;
        }
        let Ok(okunan) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in okunan.flatten() {
            let Ok(m) = e.metadata() else {
                continue;
            };
            let yol = e.path();
            if m.is_dir() {
                yigin.push(yol);
            } else if m.is_file() && yol.extension().map(|x| x == "log").unwrap_or(false) {
                let t = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                if t >= sinir {
                    bulunan.push((t, yol));
                }
            }
        }
    }
    bulunan.sort_by(|a, b| b.0.cmp(&a.0));
    bulunan.into_iter().map(|(_, y)| y).collect()
}

/// Verinin son `sinir` baytı (satır başından başlatılır).
pub fn kuyruk(veri: &[u8], sinir: usize) -> &[u8] {
    if veri.len() <= sinir {
        return veri;
    }
    let bas = veri.len() - sinir;
    let parca = &veri[bas..];
    match parca.iter().position(|&b| b == b'\n') {
        Some(i) if i + 1 < parca.len() => &parca[i + 1..],
        _ => parca,
    }
}

/// Dosyaları başlıklı tek metinde toplayıp gzip ile sıkıştırır.
pub fn paketle(baslik: &str, dosyalar: &[(String, Vec<u8>)]) -> ResultType<Vec<u8>> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz.write_all(baslik.as_bytes())?;
    gz.write_all(b"\n")?;
    let mut toplam = 0usize;
    for (ad, veri) in dosyalar {
        let parca = kuyruk(veri, DOSYA_SINIRI);
        if toplam + parca.len() > TOPLAM_SINIR {
            writeln!(gz, "\n===== {} atlandi (toplam sinir) =====", ad)?;
            continue;
        }
        toplam += parca.len();
        writeln!(gz, "\n===== {} ({} / {} bayt) =====", ad, parca.len(), veri.len())?;
        gz.write_all(parca)?;
    }
    Ok(gz.finish()?)
}

/// Kayıt klasörlerinden paketi hazırlar. `kokler`: (etiket, klasör); dosya adları
/// `etiket/göreli yol` olarak yazılır. Hiç dosya yoksa hata.
pub fn hazirla(kokler: &[(&str, &Path)], baslik: &str) -> ResultType<Vec<u8>> {
    let mut dosyalar = Vec::new();
    for (etiket, kok) in kokler {
        for yol in dosyalari_sec(kok, SystemTime::now(), GUN) {
            if let Ok(veri) = std::fs::read(&yol) {
                let goreli = yol.strip_prefix(kok).unwrap_or(&yol).to_string_lossy().to_string();
                dosyalar.push((format!("{}/{}", etiket, goreli), veri));
            }
        }
    }
    if dosyalar.is_empty() {
        crate::bail!("gonderilecek kayit dosyasi bulunamadi");
    }
    paketle(baslik, &dosyalar)
}

/// Programdan hizmete verilen hazır paketin ve gönderilen gövdenin üst sınırı.
pub const EK_SINIRI: usize = 8 * 1024 * 1024;

/// gzip başlığı (1f 8b) ile mi başlıyor.
#[inline]
pub fn gzip_mi(veri: &[u8]) -> bool {
    veri.len() > 2 && veri[0] == 0x1f && veri[1] == 0x8b
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn kuyruk_satir_basindan() {
        assert_eq!(kuyruk(b"abc", 10), b"abc");
        assert_eq!(kuyruk(b"satir1\nsatir2\nson", 8), b"son", "yarım satır atılır");
        assert_eq!(kuyruk(b"uzunsatirsatirsonu", 5), b"rsonu", "satır sonu yoksa son baytlar");
    }

    #[test]
    fn iki_paket_art_arda_tek_dosya_gibi_acilir() {
        let a = paketle("hizmet", &[("h.log".into(), b"bir\n".to_vec())]).unwrap();
        let b = paketle("program", &[("p.log".into(), b"iki\n".to_vec())]).unwrap();
        assert!(gzip_mi(&a) && gzip_mi(&b) && !gzip_mi(b"duz metin"));
        let mut govde = a.clone();
        govde.extend(&b);
        let mut s = String::new();
        flate2::read::MultiGzDecoder::new(&govde[..]).read_to_string(&mut s).unwrap();
        assert!(s.contains("bir") && s.contains("iki"), "zcat/gunzip ikisini de açar");
    }

    #[test]
    fn paket_acilinca_icerik_dogru() {
        let p = paketle("AltunakarDesk 1.5.0-12 / 123", &[("a.log".into(), b"merhaba\n".to_vec())]).unwrap();
        let mut s = String::new();
        flate2::read::GzDecoder::new(&p[..]).read_to_string(&mut s).unwrap();
        assert!(s.starts_with("AltunakarDesk 1.5.0-12 / 123\n"));
        assert!(s.contains("===== a.log (8 / 8 bayt) =====\nmerhaba\n"));
    }

    #[test]
    fn eski_ve_log_olmayan_dosya_secilmez() {
        let k = std::env::temp_dir().join(format!("altunakar-kayit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&k);
        std::fs::create_dir_all(k.join("server")).unwrap();
        std::fs::write(k.join("AltunakarDesk_rCURRENT.log"), b"x").unwrap();
        std::fs::write(k.join("server").join("s.log"), b"y").unwrap();
        std::fs::write(k.join("notlar.txt"), b"z").unwrap();
        let simdi = SystemTime::now();
        let secilen = dosyalari_sec(&k, simdi, GUN);
        assert_eq!(secilen.len(), 2, "alt klasör dahil yalnız .log");
        let ileride = simdi + Duration::from_secs(10 * 24 * 3600);
        assert!(dosyalari_sec(&k, ileride, GUN).is_empty(), "3 günden eski seçilmez");
        assert!(hazirla(&[("x", &k.join("yok"))], "b").is_err(), "dosya yoksa hata");
        let p = hazirla(&[("kullanici", &k), ("yok", &k.join("yok"))], "b").unwrap();
        let mut s = String::new();
        flate2::read::GzDecoder::new(&p[..]).read_to_string(&mut s).unwrap();
        assert!(s.contains("===== kullanici/AltunakarDesk_rCURRENT.log"), "etiketli ad");
        std::fs::remove_dir_all(&k).ok();
    }
}
