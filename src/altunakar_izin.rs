//! Altunakar: erişim türüne göre izin profilleri.
//!
//! İki profil var. "Etkileşimli" (tek seferlik şifre ya da "Kabul et") bugünkü izin ayarlarını
//! (`enable-*`) kullanır. "Gözetimsiz" (kalıcı şifreyle, kimse onaylamadan) için her izin ayrıca
//! `altunakar-gozetimsiz-enable-*` anahtarında tutulabilir; tutulmamışsa etkileşimli ayar geçerli
//! (mevcut kurulumlarda hiçbir şey değişmez).

use crate::config::option2bool;

pub const ONEK: &str = "altunakar-gozetimsiz-";

/// Profilin kapsadığı oturum izinleri (bağlantı penceresinde açılıp kapanabilenler).
pub const IZINLER: [&str; 8] = [
    "enable-keyboard",
    "enable-clipboard",
    "enable-audio",
    "enable-file-transfer",
    "enable-remote-restart",
    "enable-record-session",
    "enable-block-input",
    "enable-privacy-mode",
];

#[inline]
pub fn anahtar(izin: &str) -> String {
    format!("{}{}", ONEK, izin)
}

/// Gözetimsiz oturumda iznin değeri: profilde ayar yoksa temel (etkileşimli) değer.
pub fn profil_izni(izin: &str, profil_degeri: &str, temel: bool) -> bool {
    if profil_degeri.is_empty() {
        temel
    } else {
        option2bool(izin, profil_degeri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profil_yoksa_temel_gecerli() {
        assert!(profil_izni("enable-file-transfer", "", true));
        assert!(!profil_izni("enable-file-transfer", "", false));
    }

    #[test]
    fn profil_temeli_ezer() {
        assert!(!profil_izni("enable-file-transfer", "N", true), "gözetimsizde dosya kapatılabilir");
        assert!(profil_izni("enable-keyboard", "Y", false), "gözetimsizde açılabilir");
        assert_eq!(anahtar("enable-audio"), "altunakar-gozetimsiz-enable-audio");
        assert!(IZINLER.iter().all(|i| i.starts_with("enable-")), "option2bool anlamı korunur");
    }
}
