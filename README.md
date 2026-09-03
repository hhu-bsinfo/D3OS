# Service Management für D3OS

## Automatischer Start
Der Service-Manager wird aus `boot.rs` heraus automatisch gestartet. Dabei wird ihm das Boot Target `--terminal` oder `--gui` übergeben, je nachdem, wie `BOOT_TO_GUI` gesetzt ist.

## Manifeste
Jeder Service benötigt ein JSON-Manifest, welches unter `/loader/initrd/services/` gespeichert sein muss. Für das Terminal und die GUI wurden bereits Manifeste angelegt. Diese können als Vorlage für weitere Manifeste dienen. 

## Wie funktioniert die Absturzerkennung?
Es gibt zwei Mechanismen, um Abstürze zu erkennen:
1. Exit-Codes: Wenn ein Programm mit einem Fehler abstürzt (z.B. durch eine Panic), merkt der Manager das sofort und startet ggf. neu (je nach Restart Policy).
2. Heartbeat-Nachrichten: Wenn im JSON-Manifest "heartbeat" : true gesetzt wird, erwartet der Service-Manager alle zwei Sekunden eine Nachricht des Services. Hat er fünf Sekunden lang keine erhalten, geht er von einem Deadlock aus und terminiert den Service. Sollen Heartbeat-Nachrichten genutzt werden, muss sichergestellt werden, dass der Service diese korrekt sendet.

## Systemstatus anzeigen
Es wurde ein kleiner Task-Manager entwickelt, welcher eine Übersicht über alle Services sowie die Prozesse und den Speicher gibt. Dieser kann mit `task_manager` im Terminal gestartet werden. 

## Eigene Tests durchführen
Um den Service-Manager zu testen, liegt ein Dummy-Service bereit. Dieser kann mit mehreren Instanzen und unterschiedlichen Abhängigkeiten/Argumenten gestartet werden.
So kann beispielsweise das Argument `--panic-after-ms=5000` verwendet werden, um eine Panic nach fünf Sekunden zu erzeugen, und dann kann geprüft werden, ob die Exit-Codes sowie die Neustart-Logik korrekt funktionieren.
Es befinden sich bereits einige Manifest-Dateien für den Dummy Service im angegebenen Ordner, welche zum Testen wieder aktiviert werden können. 


