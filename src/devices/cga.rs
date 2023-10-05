/****************************************************************************
 *                                                                          *
 *                                    c g a                                 *
 *                                                                          *
 *--------------------------------------------------------------------------*
 * Beschreibung:    Mit Hilfe dieses Moduls kann man auf den Textbildschirm *
 *                  des PCs zugreifen. Der Zugriff erfolgt direkt auf der   * 
 *                  Hardwareebene, d.h. ueber den Bildschirmspeicher und    *
 *                  den I/O-Ports der Grafikkarte.                          *
 *                                                                          *
 * Autor:           Michael Schoetter, HHU Duesseldorf, 27.12.2021          *
 ****************************************************************************/


use crate::kernel::cpu as cpu;


// make type comparable, printable and enable copy semantics
#[allow(dead_code)]   // avoid warnings for unused colors
#[repr(u8)]           // store each enum variant as an u8
pub enum Color {
    Black      = 0,
    Blue       = 1,
    Green      = 2,
    Cyan       = 3,
    Red        = 4,
    Pink       = 5,
    Brown      = 6,
    LightGray  = 7,
    DarkGray   = 8,
    LightBlue  = 9,
    LightGreen = 10,
    LightCyan  = 11,
    LightRed   = 12,
    LightPink  = 13,
    Yellow     = 14,
    White      = 15,
}


pub const CGA_STD_ATTR: u8       = (Color::Black as u8) << 4 | (Color::Green as u8);

const CGA_BASE_ADDR: u32     = 0xb8000;
const CGA_ROWS   : u32       = 25;
const CGA_COLUMNS: u32       = 80;

const CGA_INDEX_PORT: u16    = 0x3d4;  // select register
const CGA_DATA_PORT: u16     = 0x3d5;  // read/write register
const CGA_HIGH_BYTE_CMD: u8  = 14;     // cursor high byte
const CGA_LOW_BYTE_CMD: u8   = 15;     // cursor high byte


/*****************************************************************************
 * Funktion:        clear                                                    *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Lösche den Textbildschirm.                               *
 *****************************************************************************/
pub fn clear() {

   /* Hier muss Code eingefuegt werden */

}


/*****************************************************************************
 * Funktion:        show                                                     *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Anzeige eines Zeichens mit Attribut an einer bestimmten  *
 *                  Stelle auf dem Bildschirm.                               *
 *                                                                           *
 * Parameter:                                                                *
 *      x,y         Position des Zeichens                                    *
 *      character   Das auszugebende Zeichen                                 *
 *      attrib      Attributbyte fuer das Zeichen                            *
 *****************************************************************************/
pub fn show (x: u32, y: u32, character: char, attrib: u8) {
    let pos: u32;

    if x>CGA_COLUMNS || y>CGA_ROWS
    {    
		return ; 
    }
    
    pos = (y * CGA_COLUMNS + x) * 2;

    unsafe {
        *((CGA_BASE_ADDR + pos) as *mut u8)     = character as u8;
        *((CGA_BASE_ADDR + pos + 1) as *mut u8) = attrib;
    }
}


/*****************************************************************************
 * Funktion:        getpos                                                   *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Abfragem der Cursorposition                              *
 *                                                                           *
 * Rückgabewerte:   x und y                                                  *
 *****************************************************************************/
pub fn getpos () -> (u32, u32) {

   /* Hier muss Code eingefuegt werden */

   (0,0) // Platzhalter, entfernen und durch sinnvollen Rueckgabewert ersetzen 
}


/*****************************************************************************
 * Funktion:        setpos                                                   *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Setzen des Cursors in Spalte x und Zeile y.              *
 *****************************************************************************/
pub fn setpos (x:u32, y:u32) {

   /* Hier muss Code eingefuegt werden */

}


/*****************************************************************************
 * Funktion:        print_dec                                                *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Ausgabe eines u32 Wertes als Dezimal-Zahl, ohne führende *
 *                  Null, an der aktuellen Cursorposition mit dem Standard-  *
 *                  Attribut.                                                *
 *                                                                           *
 * Parameter:       zahl       auszugebende Hex-Zahl                         *
 *****************************************************************************/
pub fn print_dec (mut zahl: u32) {

   /* Hier muss Code eingefuegt werden */

}

 
/*****************************************************************************
 * Funktion:        print_hex                                                *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Ausgabe eines u32 Wertes als Hex-Zahl, ohne führende     *
 *                  Null, an der aktuellen Cursorposition mit dem Standard-  *
 *                  Attribut.                                                *
 *                                                                           *
 * Parameter:       zahl       auszugebende Hex-Zahl                         *
 *****************************************************************************/
pub fn print_hex (zahl: u32) {

   /* Hier muss Code eingefuegt werden */

}

 
/*****************************************************************************
 * Funktion:        print_byte                                               *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Ausgabe eines Bytes an aktuellen Cursorposition mit dem  *
 *                  Standard-Attribut. '\n' fuer Zeilenvorschub.             *
 *                                                                           *
 * Parameter:       b       auszugebendes Zeichen                            *
 *****************************************************************************/
pub fn print_byte (b: u8) {

   /* Hier muss Code eingefuegt werden */

}


/*****************************************************************************
 * Funktion:        print_str                                                *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Ausgabe einer Zeichenkette, ab der aktuellen Cursor-     *
 *                  position. '\n' fuer Zeilenvorschub.                      *
 *                                                                           *
 * Parameter:       string      Auszugebende Zeichenkette                    *
 *                  attrib      Attributbyte fuer alle Zeichen der Z.kette   *
 *****************************************************************************/
pub fn print_str (string: &str, attrib: u8) {

   /* Hier muss Code eingefuegt werden */

}
    

/*****************************************************************************
 * Funktion:        scrollup                                                 *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Verschiebt den Bildschirminhalt um eine Zeile nach oben. *
 *                  Die neue Zeile am unteren Bildrand wird mit Leerzeichen  *
 *                  gefuellt.                                                *
 *****************************************************************************/
pub fn scrollup () {

   /* Hier muss Code eingefuegt werden */

}
 
 
/*****************************************************************************
 * Funktion:        attribute                                                *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Hilfsfunktion zur Erzeugung eines Attribut-Bytes aus     *
 *                  Hintergrund- und Vordergrundfarbe und der Angabe, ob das *
 *                  Zeichen blinkend darzustellen ist.                       *
 *                                                                           *
 * Parameter:       bg          Background color                             *
 *                  fg          Foreground color                             *
 *                  blink       yes/no                                       *
 *                                                                           *
 * Rückgabewert:    u8          Attribut-Code                                *
 *****************************************************************************/
pub fn attribute (bg: Color, fg: Color, blink: bool) -> u8 {

   /* Hier muss Code eingefuegt werden */
   
   0 // Platzhalter, entfernen und durch sinnvollen Rueckgabewert ersetzen 
}
