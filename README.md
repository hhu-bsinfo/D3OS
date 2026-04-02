<div align="center">
  <img src="https://coconucos.cs.hhu.de/lehre/bigdata/resources/img/hhu-logo.svg" width=300>

  [![Download](https://img.shields.io/static/v1?label=&message=pdf&color=EE3F24&style=for-the-badge&logo=adobe-acrobat-reader&logoColor=FFFFFF)](/document/Masterarbeit.pdf)
</div>

# Entwicklung einer Capability-basierten Zugriffskontrolle in einem Rust-Betriebssystem

Die vorliegende Arbeit untersucht und implementiert eine Capability-basierte Zugriffskontrolle für das Forschungsbetriebssystem D3OS, das als verteiltes, in Rust entwickeltes Mikrokernel-System für moderne Rechenzentrumsanforderungen konzipiert ist. 

Die Motivation der Arbeit liegt darin, ein flexibles und sicheres Zugriffskontrollmodell zu entwickeln, das den Anforderungen moderner, verteilter Systeme gerecht wird. Capability-basierte Zugriffskontrolle verfolgt dabei einen dezentralen Ansatz: Zugriffsrechte werden nicht global verwaltet, sondern als nicht manipulierbare Objekte direkt an Prozesse vergeben. Diese Capabilities definieren präzise, auf welche Ressourcen und in welcher Weise zugegriffen werden darf. Dadurch wird das Prinzip der minimalen Rechtevergabe konsequent umgesetzt und die Angriffsfläche reduziert.

Im theoretischen Teil werden Grundlagen der Zugriffskontrolle sowie bestehende Systeme wie seL4, NOVA oder Fuchsia analysiert. Diese dienen als Referenz für Designentscheidungen, insbesondere hinsichtlich Capability-Spaces, Rechteweitergabe und Systemintegration. Zudem werden zentrale Herausforderungen wie Revocation, also der Entzug von Rechten, und sichere Delegation behandelt.

Der praktische Schwerpunkt der Arbeit liegt auf der Implementierung des Capability-Modells in D3OS. Hierzu wurden zentrale Systemkomponenten angepasst. Capabilities werden als generische Datenstrukturen implementiert, die sowohl das geschützte Objekt als auch zugehörige Zugriffsrechte kapseln. Alle Capabilities eines Prozesses werden in einem Capability-Space verwaltet, der im Kernel liegt, um Manipulationen zu verhindern. Prozesse und Threads greifen ausschließlich über diese Capabilities auf System-Ressourcen zu.

Ein wesentlicher Beitrag ist die Umstellung der Systemaufrufe: Statt über globale Syscall-Nummern werden Aufrufe nun über Capability-Referenzen gesteuert. Dadurch kann ein Prozess nur noch solche Operationen ausführen, für die er explizit berechtigt ist. Auch das Naming-System wurden entsprechend angepasst, sodass Zugriffe nur noch über vorhandene Capabilities möglich sind. Klassische globale Sichtweisen auf Ressourcen werden dadurch eingeschränkt. Zusätzlich wurden Mechanismen zur Weitergabe und zum Entzug von Capabilities implementiert.

Die Ergebnisse zeigen, dass sich eine Capability-basierte Zugriffskontrolle effektiv in ein modernes Betriebssystem integrieren lässt und eine feingranulare sowie sichere Rechteverwaltung ermöglicht. Gleichzeitig ergeben sich neue Herausforderungen, etwa bei der sicheren Kommunikation zwischen Prozessen oder der effizienten Verwaltung von Revocations. Insgesamt bildet die Implementierung eine solide Grundlage für weiterführende Entwicklungen, insbesondere im Hinblick auf Restartability, Message-Passing und verteilte Systeme.