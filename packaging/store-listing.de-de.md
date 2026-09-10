# Store listing text, German

The German half of `store-listing.md`, one file per language. The headings are
that file's headings and stay in English, because `fenster`'s parser reads both
files the same way; only what sits under them is German. Only the fields a store
shows a reader are here.

**Terminology is the application's own, out of `crates/segler-desktop/po/de.po`** — a listing that
calls a thing something the window does not teaches the customer a word the
product has no use for. Where the catalogue has a term, it wins.

**German runs longer than English.** Run `fenster/check-listing.ps1` on this file
after any edit to either language rather than trusting a translation to fit.
## Subtitle (Mac App Store, 30)

DocLang-Dokumente bearbeiten

## Promotional text (Mac App Store, 170)

Ein DocLang-Dokument öffnen, es lesen, wie ein Leser es läse, es an Ort und Stelle verbessern und dieselbe Datei mit Ihren Änderungen speichern, sonst unverändert.

## Short description (Microsoft Store, 500)

DocLang ist das offene Auszeichnungsformat für Dokumente, die Sprachmodelle lesen und schreiben. Das meiste ist die Lesart eines Modells von einem PDF oder Scan, mitunter falsch.

Segler zeigt ein Dokument, wie ein Leser es sähe, und lässt Sie es dort verbessern: eine falsch gelesene Zeile, die Ebene einer Überschrift, eine Zelle. Trägt das Archiv Seitenbilder, prüfen Sie daran eine zweifelhafte Zeile.

Speichern schreibt die geöffnete Datei mit Ihren Änderungen und sonst nichts.

## App features (Microsoft Store, up to 20 bullets of 200 characters)

    Das Dokument, wie ein Leser es sieht: Überschriften, Absätze, Listen, Tabellen und Bilder in der Lesereihenfolge.
    Text dort bearbeiten, wo Sie ihn lesen: einen Absatz, einen Listeneintrag oder eine Zelle an ihrem Platz.
    Fett und Kursiv stehen beim Tippen als Auszeichnungen da, damit eine Korrektur sie behält und Formatierung hinzukommen kann.
    Die Struktur neben dem Dokument, und die Eigenschaften des gewählten Elements in Reichweite: Art, Klasse, Ebene, Beschriftung, Lage, Rahmen und die Art einer Tabellenzelle.
    Seitenbilder neben dem Dokument, wenn das Archiv sie trägt, mit den verorteten Rahmen darüber gezeichnet.
    Rückgängig und Wiederholen für jede Änderung.
    Prüfung gegen die DocLang-Spezifikation, jedes Problem einen Klick von seinem Element entfernt.
    Speichert die geöffnete Datei mit Ihren Änderungen, sonst unverändert. Auszeichnung, die Sie nicht angefasst haben, wird Byte für Byte zurückgeschrieben.
    Keinerlei Netzwerkverbindung. Kein Konto, keine Telemetrie, nichts wird irgendwohin gesendet.
    Open Source, und ebenso das Format, das es bearbeitet.

## Description (both, written to 4,000)

DocLang ist das offene Auszeichnungsformat für Dokumente, die Sprachmodelle lesen und schreiben: der Aufbau, der Text, das Layout und die Lesereihenfolge eines Dokuments in einer Datei, von der LF AI & Data Foundation. Das meiste DocLang stammt von einem Modell aus einem PDF oder einem Scan, und ein Modell irrt sich mitunter: eine falsch gelesene Zeile, eine Überschrift, die für einen Absatz gehalten wurde, eine Tabellenzelle in der falschen Spalte.

Segler ist ein Editor für solche Dokumente.

WAS SIE SEHEN

Das Dokument, wie ein Leser es sähe: Überschriften, Absätze, Listen, Tabellen und Bilder in der Lesereihenfolge. Daneben die Struktur, damit der Aufbau, den das Modell gefunden hat, neben dem Text steht, den es gefunden hat. Die Eigenschaften des gewählten Elements: seine Art, seine Ebene, seine Beschriftung, seine Lage, sein Rahmen auf der Seite. Und wenn das Archiv die Seitenbilder trägt, die das Modell gelesen hat, diese Bilder neben dem Dokument, mit den verorteten Rahmen darüber gezeichnet, sodass sich eine zweifelhafte Zeile an der Seite prüfen lässt.

WAS SIE TUN KÖNNEN

Eine Zeile dort neu tippen, wo Sie sie lesen — einen Absatz, einen Listeneintrag, eine Tabellenzelle. Trägt eine Zeile Fett oder Kursiv, stehen ihre Auszeichnungen im Feld, damit eine Korrektur sie behält und damit Formatierung hinzukommen kann. Die Ebene einer Überschrift ändern, oder die Klasse einer Liste oder eines Bildes. Die Art einer Zelle von Zelle auf Spaltenkopf ändern. Einen Rahmen berichtigen oder löschen. Die Beschriftung oder die Lage eines Elements ändern. Ein Element entfernen, das dort nicht hingehört. Jede Änderung lässt sich rückgängig machen.

Segler prüft das Dokument während der Arbeit gegen die DocLang-Spezifikation und listet auf, was es findet; jedes Problem ist einen Klick von seinem Element entfernt.

WAS EIN SPEICHERN TUT

Speichern schreibt die geöffnete Datei mit Ihren Änderungen und sonst nichts anderem. Auszeichnung, die Sie nicht angefasst haben, wird Byte für Byte zurückgeschrieben, samt Kommentaren und Leerraum. Die Seitenbilder eines Archivs und alles Übrige darin werden unangetastet übernommen. Ein Dokument, das Sie nicht geändert haben, wird gar nicht erst neu geschrieben.

WAS ES NICHT TUT

Keinerlei Netzwerkverbindung. Kein Konto. Keine Telemetrie, keine Analyse, keine Absturzberichte. Nichts über Sie oder Ihre Dokumente wird irgendwohin gesendet, weil es nirgendwohin zu senden gibt.

Es wandelt nicht um. Segler bearbeitet DocLang, das es bereits gibt; DocLang aus einem PDF herzustellen ist die Aufgabe eines Konverters.

OPEN SOURCE

Segler ist Open Source unter derselben Lizenz wie DocLang selbst, und die Bibliothek darunter ist eine eigene Crate, auf der jeder aufbauen kann: github.com/excelano/segler.

## Release notes

*Neu in dieser Version*, aus `CHANGELOG.md`, neueste zuerst. Nur die Fassung,
die eingereicht wird, braucht einen Abschnitt; 0.1.0 und 0.1.1 haben keinen,
weil kein Store Segler je ausgeliefert hat und es niemanden gibt, der von
ihnen aufrüstet.

### 0.1.2

Erste Veröffentlichung.

## Keywords

**Mac App Store** (100 characters, comma-separated, no spaces after commas):

    DocLang,dclx,dclg,Dokument,Editor,Auszeichnung,XML,docling,Layout,Korrektur

**Microsoft Store** (seven terms):

    DocLang, dclx, Dokumenteditor, Auszeichnung, XML, docling, Layout
