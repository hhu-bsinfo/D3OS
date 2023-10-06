/*****************************************************************************
 *                                                                           *
 *                                  k e y b o a r d                          *
 *                                                                           *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Treiber für den Tastaturcontroller des PCs.              *
 *                                                                           *
 * Autor:           Michael Schoettner, HHU Duesseldorf, 18.1.2022           *
 *****************************************************************************/

#![allow(unused_variables)]
#![allow(dead_code)]

use spin::Mutex;
use x86_64::instructions::port::Port;

use crate::devices::key as key;  // shortcut for key


// Public functions for convenient access 
pub fn key_hit() -> key::Key {
    return KB.lock().key_hit();
}


// Global thread-safe access to keyboard
static KB: Mutex<Keyboard> = Mutex::new(
    Keyboard {
        code: 0,
        prefix: 0,
        gather: key::Key { asc: 0, scan: 0, modi: 0 },
        leds: 0,
    });

// Defining Keyboard struct 
struct Keyboard {
    code: u8,
    // Byte von Tastatur
    prefix: u8,
    // Prefix von Tastatur
    gather: key::Key,
    // letzter dekodierter Key
    leds: u8,          // Zustand LEDs
}

/* Tabellen fuer ASCII-Codes intiialisieren */
static NORMAL_TAB: [u8; 89] =
    [
        0, 0, 49, 50, 51, 52, 53, 54, 55, 56, 57, 48, 225, 39, 8, 0, 113,
        119, 101, 114, 116, 122, 117, 105, 111, 112, 129, 43, 13, 0, 97,
        115, 100, 102, 103, 104, 106, 107, 108, 148, 132, 94, 0, 35, 121,
        120, 99, 118, 98, 110, 109, 44, 46, 45, 0, 42, 0, 32, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 45, 0, 0, 0, 43, 0, 0, 0, 0,
        0, 0, 0, 60, 0, 0
    ];

static SHIFT_TAB: [u8; 89] =
    [
        0, 0, 33, 34, 21, 36, 37, 38, 47, 40, 41, 61, 63, 96, 0, 0, 81,
        87, 69, 82, 84, 90, 85, 73, 79, 80, 154, 42, 0, 0, 65, 83, 68,
        70, 71, 72, 74, 75, 76, 153, 142, 248, 0, 39, 89, 88, 67, 86, 66,
        78, 77, 59, 58, 95, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 62, 0, 0
    ];

static ALT_TAB: [u8; 89] =
    [
        0, 0, 0, 253, 0, 0, 0, 0, 123, 91, 93, 125, 92, 0, 0, 0, 64, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 126, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 230, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 124, 0, 0
    ];

static ASC_NUM_TAB: [u8; 13] = [55, 56, 57, 45, 52, 53, 54, 43, 49,
    50, 51, 48, 44
];

static SCAN_NUM_TAB: [u8; 13] = [8, 9, 10, 53, 5, 6, 7, 27, 2,
    3, 4, 11, 51
];

// Namen der LEDs
const LED_CAPS_LOCK: u8 = 4;
const LED_NUM_LOCK: u8 = 2;
const LED_SCROLL_LOCK: u8 = 1;

// Konstanten fuer die Tastaturdekodierung
const BREAK_BIT: u8 = 0x80;
const PREFIX1: u8 = 0xe0;
const PREFIX2: u8 = 0xe1;

// Bits im Statusregister des Tastaturcontrollers
const KBD_OUTB: u8 = 0x01;
const KBD_INPB: u8 = 0x02;
const KBD_AUXB: u8 = 0x20;

// Kommandos an die Tastatur
const KBD_CMD_SET_LED: u8 = 0xed;
const KBD_CMD_SET_SPEED: u8 = 0xf3;
const KBD_CMD_CPU_RESET: u8 = 0xfe;

// Antworten der Tastatur
const KBD_REPLY_ACK: u8 = 0xfa;

static mut CONTROL_PORT: Port<u8> = Port::new(0x64);
static mut DATA_PORT: Port<u8> = Port::new(0x60);

impl Keyboard {
    /*****************************************************************************
     * Funktion:        key_decoded                                              *
     *---------------------------------------------------------------------------*
     * Beschreibung:    Interpretiert die Make- und Break-Codes der Tastatur.    *
     *                                                                           *
     * Rueckgabewert:   true bedeutet, dass das Zeichen komplett ist             *
     *                  false es fehlen noch Make- oder Break-Codes.             *
     *****************************************************************************/
    fn key_decoded(&mut self) -> bool {
        let mut done: bool = false;

        // Die Tasten, die bei der MF II Tastatur gegenueber der aelteren
        // AT Tastatur hinzugekommen sind, senden immer erst eines von zwei
        // moeglichen Prefix Bytes.
        if self.code == PREFIX1 || self.code == PREFIX2 {
            self.prefix = self.code;
            return false;
        }

        // Das Loslassen einer Taste ist eigentlich nur bei den "Modifier" Tasten
        // SHIFT, CTRL und ALT von Interesse, bei den anderen kann der Break-Code
        // ignoriert werden.
        if (self.code & BREAK_BIT) != 0 {
            self.code &= !BREAK_BIT;     // Der Break-Code einer Taste ist gleich dem
            // Make-Code mit gesetzten break_bit.
            match self.code {
                42 | 54 => {
                    self.gather.set_shift(false);
                }
                56 => {
                    if self.prefix == PREFIX1 { self.gather.set_alt_right(false); } else { self.gather.set_alt_left(false); }
                }
                29 => {
                    if self.prefix == PREFIX1 { self.gather.set_ctrl_right(false); } else { self.gather.set_ctrl_left(false); }
                }
                _ => { // alle anderen Tasten
                }
            }

            // Ein Prefix gilt immer nur fuer den unmittelbar nachfolgenden Code.
            // Also ist es jetzt abgehandelt.
            self.prefix = 0;

            // Mit einem Break-Code kann man nichts anfangen, also false liefern.
            return false;
        }

        // Eine Taste wurde gedrueckt. Bei den Modifier Tasten wie SHIFT, ALT,
        // NUM_LOCK etc. wird nur der interne Zustand geaendert. Durch den
        // Rueckgabewert 'false' wird angezeigt, dass die Tastatureingabe noch
        // nicht abgeschlossen ist. Bei den anderen Tasten werden ASCII
        // und Scancode eingetragen und ein 'true' fuer eine erfolgreiche
        // Tastaturabfrage zurueckgegeben, obwohl genaugenommen noch der Break-
        // code der Taste fehlt.

        match self.code {
            42 | 54 => {
                self.gather.set_shift(true);
            }
            56 => {
                if self.prefix == PREFIX1 { self.gather.set_alt_right(true); } else { self.gather.set_alt_left(true); }
            }
            29 => {
                if self.prefix == PREFIX1 { self.gather.set_ctrl_right(true); } else { self.gather.set_ctrl_left(true); }
            }
            58 => {
                self.gather.set_caps_lock(!self.gather.get_caps_lock());
                self.set_led(LED_CAPS_LOCK, self.gather.get_caps_lock());
            }
            70 => {
                self.gather.set_scroll_lock(!self.gather.get_scroll_lock());
                self.set_led(LED_SCROLL_LOCK, self.gather.get_scroll_lock());
            }
            69 => { // Numlock oder Pause ?
                if self.gather.get_ctrl_left() { // Pause Taste
                    // Auf alten Tastaturen konnte die Pause-Funktion wohl nur
                    // ueber Ctrl+NumLock erreicht werden. Moderne MF-II Tastaturen
                    // senden daher diese Codekombination, wenn Pause gemeint ist.
                    // Die Pause Taste liefert zwar normalerweise keinen ASCII-
                    // Code, aber Nachgucken schadet auch nicht. In jedem Fall ist
                    // die Taste nun komplett.
                    self.get_ascii_code();
                    done = true;
                } else { // NumLock
                    self.gather.set_num_lock(!self.gather.get_num_lock());
                    self.set_led(LED_NUM_LOCK, self.gather.get_num_lock());
                }
            }

            _ => { // alle anderen Tasten
                // ASCII-Codes aus den entsprechenden Tabellen auslesen, fertig.
                self.get_ascii_code();
                done = true;
            }
        }

        // Ein Prefix gilt immer nur fuer den unmittelbar nachfolgenden Code.
        // Also ist es jetzt abgehandelt.
        self.prefix = 0;

        if done { return true; }   // Tastaturabfrage abgeschlossen
        else { return false; }
    }


    /*****************************************************************************
     * Funktion:        get_ascii_code                                           *
     *---------------------------------------------------------------------------*
     * Beschreibung:    Ermittelt anhand von Tabellen aus dem Scancode und den   *
     *                  gesetzten Modifier-Bits den ASCII-Code der Taste.        *
     *****************************************************************************/
    fn get_ascii_code(&mut self) {

        // Sonderfall Scancode 53: Dieser Code wird sowohl von der Minustaste
        // des normalen Tastaturbereichs, als auch von der Divisionstaste des
        // Ziffernblocks gesendet. Damit in beiden Faellen ein Code heraus-
        // kommt, der der Aufschrift entspricht, muss im Falle des Ziffern-
        // blocks eine Umsetzung auf den richtigen Code der Divisionstaste
        // erfolgen.
        if self.code == 53 && self.prefix == PREFIX1 { // Divisionstaste des Ziffernblocks
            self.gather.set_ascii('/' as u8);
            self.gather.set_scancode(key::SCAN_DIV);
        }

        // Anhand der Modifierbits muss die richtige Tabelle ausgewaehlt
        // werden. Der Einfachheit halber hat NumLock Vorrang vor Alt,
        // Shift und CapsLock. Fuer Ctrl gibt es keine eigene Tabelle
        else if self.gather.get_num_lock() && self.prefix == 0 &&
            self.code >= 71 && self.code <= 83 {
            // Bei eingeschaltetem NumLock und der Betaetigung einer der
            // Tasten des separaten Ziffernblocks (Codes 71-83), sollen
            // nicht die Scancodes der Cursortasten, sondern ASCII- und
            // Scancodes der ensprechenden Zifferntasten geliefert werden.
            // Die Tasten des Cursorblocks (prefix == prefix1) sollen
            // natuerlich weiterhin zur Cursorsteuerung genutzt werden
            // koennen. Sie senden dann uebrigens noch ein Shift, aber das
            // sollte nicht weiter stoeren.
            self.gather.set_ascii(ASC_NUM_TAB[(self.code - 71) as usize]);
            self.gather.set_scancode(SCAN_NUM_TAB[(self.code - 71) as usize]);
        } else if self.gather.get_alt_right() {
            self.gather.set_ascii(ALT_TAB[self.code as usize]);
            self.gather.set_scancode(self.code);
        } else if self.gather.get_shift() {
            self.gather.set_ascii(SHIFT_TAB[self.code as usize]);
            self.gather.set_scancode(self.code);
        } else if self.gather.get_caps_lock() {
            // Die Umschaltung soll nur bei Buchstaben gelten
            if (self.code >= 16 && self.code <= 26) ||
                (self.code >= 30 && self.code <= 40) ||
                (self.code >= 44 && self.code <= 50) {
                self.gather.set_ascii(SHIFT_TAB[self.code as usize]);
                self.gather.set_scancode(self.code);
            } else {
                self.gather.set_ascii(NORMAL_TAB[self.code as usize]);
                self.gather.set_scancode(self.code);
            }
        } else {
            self.gather.set_ascii(NORMAL_TAB[self.code as usize]);
            self.gather.set_scancode(self.code);
        }
    }

    /*****************************************************************************
     * Funktion:        key_hit                                                  *
     *---------------------------------------------------------------------------*
     * Beschreibung:    Diese Methode soll einen Tastendruck zurueckliefern.     *
     *                  Hierzu soll die Tastatur in einer Schleife "gepollt"     *
     *                  werden, bis ein Zeichen eingegebn wurde.                 *
     *                                                                           *
     *                  Das Byte von der Tastatur soll in dem Attribut 'code'    *
     *                  (siehe Keyboard.h) gespeichert werden. Die Dekodierung   *
     *                  soll mithilfe der vorgegebenen Funktion 'key_decoded'    *
     *                  erfolgen.                                                *
     *                                                                           *
     * Rückgabewert:    Wenn der Tastendruck abgeschlossen ist und ein Scancode, *
     *                  sowie gegebenenfalls ein ASCII-Code emittelt werden      *
     *                  konnte, werden diese in 'gather' (siehe Keyboard.h)      *
     *                  zurueckgeliefert. Anderenfalls liefert key_hit () einen  *
     *                  ungueltigen Wert zurueck, was mit Key::valid ()          *
     *                  ueberprueft werden kann.                                 *
     *****************************************************************************/
    fn key_hit(&mut self) -> key::Key {
        let invalid: key::Key = Default::default();  // nicht explizit initialisierte Tasten sind ungueltig

        /* Hier muss Code eingefuegt werden. */

        return invalid;
    }

    /*****************************************************************************
     * Funtkion:        reboot                                                   *
     *---------------------------------------------------------------------------*
     * Beschreibung:    Fuehrt einen Neustart des Rechners durch.                *
     *****************************************************************************/
    unsafe fn reboot(&mut self) {
        let mut status: u8;

        // Dem BIOS mitteilen, dass das Reset beabsichtigt war
        // und kein Speichertest durchgefuehrt werden muss.
        unsafe { *(0x472 as *mut u16) = 0x1234; }

        // Der Tastaturcontroller soll das Reset ausloesen.
        loop {
            status = CONTROL_PORT.read();
            if (status & KBD_INPB) != 0 {
                break;
            }
        };
        CONTROL_PORT.write(KBD_CMD_CPU_RESET);
    }

    /*****************************************************************************
     * Funkion:         set_repeat_rate                                          *
     *---------------------------------------------------------------------------*
     * Beschreibung:    Einstellen der Wiederholungsrate der Tastatur.           *
     *                                                                           *
     * Parameter:                                                                *
     *      delay:      Bestimmt, wie lange eine Taste gedrueckt werden muss,    *
     *                  bevor die Wiederholung einsetzt. Erlaubt sind Werte      *
     *                  zw. 0 (minimale Wartezeit) und 3 (maximale Wartezeit).   *
     *                  0=250ms, 1=500ms, 2=750ms, 3=1000ms                      *
     *                                                                           *
     *      speed:      Bestimmt, wie schnell die Tastencodes aufeinander folgen *
     *                  sollen. Erlaubt sind Werte zwischen 0 (sehr schnell)     *
     *                  und 31 (sehr langsam).                                   *
     *                                                                           *
     *                  ((2 ^ B) * (D + 8) / 240 sec                             *
     *                  Bits 4-3 = B; Bits 2-0 = D;                              *
     *****************************************************************************/
    fn set_repeat_rate(&mut self, speed: u8, delay: u8) {
        /* Hier muss Code eingefuegt werden. */
    }

    /*****************************************************************************
     * Funktion:        set_led                                                  *
     *---------------------------------------------------------------------------*
     * Beschreibung:    Setzt oder loescht die angegebene Leuchtdiode.           *
     *                                                                           *
     * Parameter:                                                                *
     *      led:        Welche LED? (caps_lock, num_lock, scroll_lock)           *
     *      on:         0 = aus, 1 = an                                          *
     *****************************************************************************/
    fn set_led(&mut self, led: u8, on: bool) {
        /* Hier muss Code eingefuegt werden. */
    }
}
