import type { Messages } from "../translations";

const de: Messages = {
  "common.loading": "Laden...",
  "common.back": "Zurück",
  "common.close": "Schließen",
  "common.cancel": "Abbrechen",
  "common.refresh": "Aktualisieren",
  "common.create": "Erstellen",
  "common.creating": "Wird erstellt...",
  "common.copied": "Kopiert!",
  "common.address": "Adresse",
  "common.currency": "Währung",
  "common.username": "Benutzername",
  "common.password": "Passwort",
  "common.status": "Status",
  "common.country": "Land",
  "common.type": "Typ",
  "common.amount": "Betrag",
  "common.login": "Anmelden",
  "common.running": "Läuft",
  "common.reconnecting": "Verbinde erneut...",
  "common.stopped": "Gestoppt",
  "common.support": "Support",
  "common.direct": "Direkt",
  "common.other": "Andere",
  "common.continue": "Weiter",

  "nav.market": "Markt",
  "nav.seller": "Verkäufer",
  "nav.faq": "FAQ",
  "nav.login": "Anmelden",
  "nav.settings": "Einstellungen",
  "nav.account": "Konto",
  "nav.discord": "Discord",

  "status.authenticated": "Authentifiziert",
  "status.notAuthenticated": "Nicht authentifiziert",

  "welcome.title": "Willkommen bei ProxyBase",
  "welcome.subtitle":
    "Ein dezentraler Peer-to-Peer-Bandbreitenmarktplatz. Kaufe und verkaufe Proxy-Zugriff mit Kryptowährungs-Einzahlungen.",
  "welcome.createWallet": "Neues Wallet erstellen",
  "welcome.importWallet": "Vorhandenes Wallet importieren",
  "welcome.createTitle": "Wallet erstellen",
  "welcome.createDesc":
    "Erstelle ein neues BIP-39-Wallet. Bewahre deine Mnemonic sicher auf.",
  "welcome.encryptionPassword": "Verschlüsselungspasswort (optional)",
  "welcome.passwordPlaceholder": "Für kein Passwort leer lassen",
  "welcome.walletCreated": "Wallet erstellt",
  "welcome.saveMnemonic": "Speichere deine Mnemonic",
  "welcome.saveMnemonicDesc":
    "Schreibe diese 12 Wörter in der richtigen Reihenfolge auf. Wer diese Phrase hat, kann auf dein Wallet zugreifen. Teile sie niemals.",
  "welcome.signingIn": "Anmeldung läuft...",
  "welcome.continue": "Weiter",
  "welcome.importTitle": "Wallet importieren",
  "welcome.importDesc":
    "Stelle dein Wallet mit einer BIP-39-Mnemonic wieder her.",
  "welcome.mnemonicPhrase": "12-Wörter-Mnemonic",
  "welcome.mnemonicPlaceholder":
    "Gib die 12 Wörter durch Leerzeichen getrennt ein...",
  "welcome.importing": "Import läuft...",
  "welcome.importAndLogin": "Importieren und anmelden",
  "welcome.autoLoginFailed":
    "Automatische Anmeldung fehlgeschlagen. Erstelle oder importiere ein Wallet.",
  "welcome.enterMnemonic": "Gib deine Mnemonic ein",

  "login.title": "Anmelden",
  "login.desc":
    "Authentifiziere dich mit deinem Wallet, um auf das ProxyBase-Netzwerk zuzugreifen.",
  "login.noWallet": "Kein Wallet gefunden",
  "login.noWalletDesc":
    "Erstelle oder importiere zuerst ein Wallet auf der Wallet-Seite.",
  "login.authenticate": "Authentifizieren",
  "login.walletAddress": "Wallet-Adresse",
  "login.walletPassword": "Wallet-Passwort",
  "login.passwordPlaceholder":
    "Gib das Wallet-Verschlüsselungspasswort ein (leer lassen, wenn keins vorhanden)",
  "login.authenticating": "Authentifizierung läuft...",
  "login.successful": "Anmeldung erfolgreich",
  "login.role": "Rolle:",
  "login.buyerAvailable": "Käufer verfügbar:",
  "login.spendableBalance": "Ausgabefähiger Saldo:",

  "wallet.title": "Wallet",
  "wallet.desc": "Verwalte deine ProxyBase-Wallet-Identität.",
  "wallet.info": "Info",
  "wallet.create": "Erstellen",
  "wallet.import": "Importieren",
  "wallet.status": "Wallet-Status",
  "wallet.addressLabel": "Adresse:",
  "wallet.loaded": "Geladen",
  "wallet.noWallet": "Kein Wallet gefunden. Erstelle oder importiere eines.",
  "wallet.clickInfo": "Klicke auf Info, um den Wallet-Status zu prüfen.",
  "wallet.createNew": "Neues Wallet erstellen",
  "wallet.encryptionPassword": "Verschlüsselungspasswort (optional)",
  "wallet.passwordPlaceholder": "Für kein Passwort leer lassen",
  "wallet.createWallet": "Wallet erstellen",
  "wallet.creating": "Wird erstellt...",
  "wallet.walletAddress": "Wallet-Adresse",
  "wallet.mnemonic": "Mnemonic — SICHER SPEICHERN",
  "wallet.mnemonicWarning":
    "Schreibe diese 12 Wörter in der richtigen Reihenfolge auf. Wer diese Phrase hat, kann auf dein Wallet zugreifen.",
  "wallet.importFromMnemonic": "Aus Mnemonic importieren",
  "wallet.mnemonicPhrase": "12-Wörter-Mnemonic",
  "wallet.mnemonicPlaceholder":
    "Gib die 12 Wörter durch Leerzeichen getrennt ein...",
  "wallet.importing": "Import läuft...",
  "wallet.importWallet": "Wallet importieren",
  "wallet.imported": "Importiert",

  "market.title": "Markt",
  "market.desc":
    "Durchstöbere Länder, Preise und verwalte Proxy-Sitzungen.",
  "market.prices": "Preise",
  "market.activeSessions": "Aktive Sitzungen",
  "market.insufficientBalance": "Nicht genügend Guthaben",
  "market.insufficientDesc":
    "Du hast nicht genügend Guthaben. Zahle Krypto ein, um fortzufahren.",
  "market.depositFunds": "Guthaben einzahlen",
  "market.dismiss": "Schließen",
  "market.connectionDetails": "Proxy-Verbindungsdetails",
  "market.remote": "Remote",
  "market.localBridge": "Lokale Brücke",
  "market.proxyAddress": "Proxy-Adresse",
  "market.sessionId": "Sitzungs-ID",
  "market.password": "Passwort",
  "market.country": "Land",
  "market.type": "Typ",
  "market.exampleCurl": "Beispiel (curl)",
  "market.localBridgeDesc":
    "Verwende die lokale Brücke für Apps wie Chrome, die keine authentifizierten Proxys unterstützen.",
  "market.auth": "Auth",
  "market.noneRequired": "Keine erforderlich",
  "market.exampleCurlLocal": "Beispiel (curl • lokal)",
  "market.bridgeNotRunning":
    "Brücke läuft nicht. Die Sitzung wurde möglicherweise auf einem anderen Gerät gekauft.",
  "market.pricing": "Preise",
  "market.refresh": "Aktualisieren",
  "market.loadingPricing": "Preisdaten werden geladen...",
  "market.category": "Kategorie",
  "market.price": "Preis",
  "market.buying": "Wird gekauft...",
  "market.buy": "Kaufen",
  "market.noSellers": "Keine Verkäufer verfügbar.",
  "market.activeSessionsCount": "Aktive Sitzungen ({count})",
  "market.noActiveSessions":
    "Keine aktiven Sitzungen. Kaufe eine im Tab Preise.",
  "market.mode": "Modus",
  "market.closeSession": "Sitzung schließen",

  "seller.title": "Verkäufer",
  "seller.desc":
    "Verkaufe deine Bandbreite auf dem ProxyBase-Marktplatz.",
  "seller.status": "Verkäufer-Status",
  "seller.refreshStatus": "Status aktualisieren",
  "seller.startStop": "Verkäufer starten / stoppen",
  "seller.includeDirect":
    "Direkt einbeziehen (eigene Bandbreite verkaufen)",
  "seller.upstreamProxies": "Upstream-Proxys (Weiterverkauf)",
  "seller.hostPort": "Host:Port",
  "seller.username": "Benutzername",
  "seller.password": "Passwort",
  "seller.remove": "Entfernen",
  "seller.addUpstream": "+ Upstream-Proxy hinzufügen",
  "seller.startSeller": "Verkäufer starten",
  "seller.stopSeller": "Verkäufer stoppen",
  "seller.activeStreams": "Aktive Streams ({count})",
  "seller.noStreams": "Keine aktiven Streams. Warte auf Verbindungen...",
  "seller.sessionId": "Sitzungs-ID",
  "seller.target": "Ziel",
  "seller.route": "Route",
  "seller.proxyN": "Proxy #{n}",

  "deposit.title": "Einzahlungen",
  "deposit.desc": "Erstelle Einzahlungen und prüfe ihren Status.",
  "deposit.create": "Einzahlung erstellen",
  "deposit.amountUsd": "Betrag ($USD)",
  "deposit.currency": "Währung",
  "deposit.createAction": "Erstellen",
  "deposit.creating": "Wird erstellt...",
  "deposit.checkStatus": "Einzahlungsstatus prüfen",
  "deposit.depositId": "Einzahlungs-ID",
  "deposit.enterDepositId": "Gib die Einzahlungs-ID ein...",
  "deposit.checkStatusAction": "Status prüfen",
  "deposit.invalidAmount": "Ungültiger Betrag",

  "depositPage.created":
    "Einzahlung erstellt — sende exakt den angezeigten Betrag",
  "depositPage.paymentQr": "Zahlungs-QR",
  "depositPage.address": "Adresse",
  "depositPage.currency": "Währung",
  "depositPage.amount": "Betrag",
  "depositPage.depositId": "Einzahlungs-ID",
  "depositPage.timeRemaining": "verbleibende Zeit",
  "depositPage.complete": "Einzahlung abgeschlossen",
  "depositPage.status": "Status",
  "depositPage.credited": "Gutschrift",
  "depositPage.done": "Fertig",
  "depositPage.expired":
    "Die Zahlungszeit ist abgelaufen. Bitte erstelle eine neue Einzahlung.",
  "depositPage.tryAgain": "Erneut versuchen",
  "depositPage.close": "Schließen",

  "account.wallet": "Wallet",
  "account.address": "Adresse",
  "account.viewBalance": "Saldo anzeigen",
  "account.manageWallet": "Wallet verwalten",
  "account.addFunds": "Guthaben hinzufügen",
  "account.noWallet": "Kein Wallet geladen.",
  "account.seller": "Verkäufer",
  "account.streamsOne": "{count} aktiver Stream",
  "account.streamsOther": "{count} aktive Streams",
  "account.sellerSettings": "Verkäufer-Einstellungen",
  "account.system": "System",
  "account.dataDir": "Datenverzeichnis",
  "account.walletPath": "Wallet",
  "account.sessionPath": "Sitzung",
  "account.configPath": "Konfiguration",
  "account.version": "Version",
  "account.logout": "Abmelden",
  "account.checkUpdate": "Nach Updates suchen",
  "account.checkingUpdate": "Suche nach Updates…",
  "account.upToDate": "Sie nutzen die neueste Version",
  "account.updateAvailable": "Neue Version v{version} verfügbar. Installieren und neu starten?",
  "account.installUpdate": "Installieren",
  "account.downloadingUpdate": "Update wird heruntergeladen…",
  "account.updateError": "Update-Prüfung fehlgeschlagen",
  "account.logoutWarning":
    "Alle aktiven Sitzungen werden geschlossen und der Verkäufer gestoppt.",
  "account.walletBalance": "Wallet-Saldo",
  "account.failedBalance": "Saldo konnte nicht geladen werden.",
  "account.appInfo": "App-Info",
  "account.spendable": "Ausgabefähig",
  "account.buyerAvailable": "Käufer verfügbar",
  "account.buyerReserved": "Käufer reserviert",
  "account.buyerSpent": "Käufer ausgegeben",
  "account.sellerPending": "Verkäufer ausstehend",
  "account.sellerAvailable": "Verkäufer verfügbar",
  "account.payoutLocked": "Auszahlung gesperrt",

  "faq.title": "FAQ",
  "faq.desc": "Häufig gestellte Fragen zu ProxyBase Markets.",
  "faq.q1": "Was ist ProxyBase Markets?",
  "faq.a1":
    "ProxyBase Markets ist ein dezentraler Peer-to-Peer-Bandbreitenmarktplatz. Verkäufer stellen ihre Internetverbindungen als Proxy-Ausgänge bereit, und Käufer erwerben Zugriff auf diese Proxys für Web-Scraping, KI-Agenten-Browsing und anderen automatisierten Datenverkehr. Alle Zahlungen werden in Microcredits abgewickelt, die durch Kryptowährungs-Einzahlungen gedeckt sind.",
  "faq.q2": "Wie beginne ich, meine Bandbreite zu verkaufen?",
  "faq.a2":
    "Gehe zum Tab Verkäufer, konfiguriere optional deine Upstream-Proxys und klicke auf „Verkäufer starten'. Dein Knoten registriert sich beim Marktplatz und erhält QoS-Sonden. Nach bestandenen Qualitätsprüfungen wird deine Verbindung nach Land und Netzwerktyp klassifiziert und steht Käufern zum Kauf zur Verfügung. Du verdienst Credits für jeden GB übertragenen Datenverkehrs.",
  "faq.q3": "Welche Netzwerktypen gibt es?",
  "faq.a3":
    "ProxyBase klassifiziert Verkäuferverbindungen in fünf Typen: Residential (Heim-ISP), Mobile (Mobilfunk), Datacenter (Cloud-/Colo-IPs), ISP (Geschäfts-/statische IPs) und Burner (VPN/Tor/Proxy-IPs). Burner-IPs werden bei QoS-Sonden markiert und können niedrigere Preise erhalten oder für bestimmte Käufergruppen gesperrt werden.",
  "faq.q4": "Wie kaufe ich eine Proxy-Sitzung?",
  "faq.a4":
    "Gehe zum Tab Markt → Preise. Suche ein Land und einen Netzwerktyp mit verfügbaren Verkäufern und klicke auf den grünen „Kaufen'-Button in dieser Zeile. Eine SOCKS5-Proxy-Sitzung wird erstellt und erscheint unter dem Tab „Aktive Sitzungen'. Dein Wallet-Saldo wird pro GB genutztem Datenverkehr belastet. Schließe eine Sitzung jederzeit über das X in ihrer Zeile.",
  "faq.q5": "Wie funktionieren Microcredits und Preise?",
  "faq.a5":
    "1.000.000 Microcredits = 1,00 USD. Die Preise werden pro Land und Netzwerktyp festgelegt (z. B. 0,50 $/GB für US-Residential). Verkäufer verdienen Credits, wenn Käufer ihre Proxys nutzen. Du kannst Guthaben mit Kryptowährungen (BTC, USDC, USDT, SOL usw.) über NOWPayments einzahlen.",
  "faq.q6": "Was ist eine Einzahlung und wie erstelle ich eine?",
  "faq.a6":
    "Öffne die Kontoseite über das Symbol im Kopfbereich und tippe auf „Guthaben hinzufügen'. Wähle einen Betrag (10, 20, 100 $ oder frei) und eine Kryptowährung. Nach der Erstellung gelangst du zu einer Einzahlungsseite mit Zahlungsadresse, QR-Code und einem 9-Minuten-Countdown. Sende exakt den angezeigten Betrag an diese Adresse – dein Saldo aktualisiert sich nach der Bestätigung automatisch.",
  "faq.q7": "Wie prüfe ich meinen Wallet-Saldo?",
  "faq.a7":
    "Gehe zur Kontoseite und tippe auf „Saldo anzeigen'. Deine Salden werden angezeigt, einschließlich ausgabefähigem Saldo, Käufer verfügbar/reserviert/ausgegeben und Verkäufer ausstehend/verfügbar/auszahlungsgesperrt.",
  "faq.q8": "Was passiert, wenn ich die App schließe?",
  "faq.a8":
    "Die App läuft im System-Tray weiter. Das Schließen des Fensters blendet sie nur aus – sie beendet sich nicht. Klicke auf das Tray-Symbol, um sie anzuzeigen oder auszublenden. Wenn du Bandbreite verkauft hast, bleibt deine Verkäufer-Sitzung bestehen und startet beim nächsten Start automatisch neu. Die App registriert sich auch für den Autostart beim Systemstart.",
  "faq.q9": "Wie erstelle ich ein Wallet?",
  "faq.a9":
    "Der Willkommensbildschirm führt dich beim ersten Start durch die Wallet-Erstellung. Um dein Wallet später zu verwalten, gehe zur Kontoseite → „Wallet verwalten'. Nutze den Tab „Erstellen', um eine neue BIP-39-Mnemonic (12 Wörter) zu generieren, oder „Importieren', um aus einer vorhandenen Phrase wiederherzustellen. Bewahre die Wörter sicher auf – sie sind der einzige Weg, dein Wallet wiederherzustellen. Deine Wallet-Daten liegen unter ~/.proxybase/.",
  "faq.q10": "Meldet sich die App automatisch an?",
  "faq.a10":
    "Ja. Beim Start erkennt der Willkommensbildschirm dein vorhandenes Wallet und meldet dich automatisch an, wenn kein Passwort gesetzt ist. Wenn du ein Wallet-Passwort festgelegt hast, musst du es auf der Anmeldeseite eingeben.",
  "faq.q11": "Welche Daten werden auf meinem Gerät gespeichert?",
  "faq.a11":
    "Alles wird unter ~/.proxybase/ gespeichert: deine verschlüsselte Wallet-Schlüsseldatei (wallet/keyfile.enc), das Sitzungstoken (session_token) und die Konfiguration (config.toml). Private Schlüssel werden nie an das Backend gesendet – die Authentifizierung nutzt kryptografische Signaturen deines lokalen Wallets.",
  "faq.q12":
    "Was sind QoS-Sonden und wie beeinflussen sie meinen Verkäuferstatus?",
  "faq.a12":
    "Quality-of-Service-Sonden sind automatisierte Tests des Marktplatzes, um Geschwindigkeit, Latenz und Zuverlässigkeit deiner Verbindung zu prüfen. Die Sonden verbinden sich durch dein Relay, messen Latenz und Verfügbarkeit und klassifizieren deine IP (Land, ISP, Netzwerktyp). Verkäufer, die bei Sonden durchfallen, werden gesperrt. Konstante gute Leistung führt zur Beförderung vom Test-Pool in die Produktion.",
  "faq.q13": "Welche Verkäufer-Pool-Stufen gibt es?",
  "faq.a13":
    "Test: neue Verkäufer in der QoS-Bewertung. Produktion: verifizierte Verkäufer mit nachgewiesener Zuverlässigkeit – sie werden für Käufersitzungen priorisiert. Gesperrt: Verkäufer, die bei Sonden durchgefallen oder offline gegangen sind – sie werden nach einer Sperrfrist automatisch neu bewertet.",
  "faq.q14":
    "Wie nutze ich den SOCKS5-Proxy nach dem Kauf einer Sitzung?",
  "faq.a14":
    "Klicke auf eine Sitzungszeile im Tab „Aktive Sitzungen', um die Verbindungsdetails zu sehen. Konfiguriere deine Anwendung so, dass sie einen SOCKS5-Proxy unter {proxyAddress} mit der Sitzungs-ID als Benutzername und deinem Sitzungstoken als Passwort verwendet. Beispiel: curl.exe --socks5 {proxyAddress} --proxy-user SITZUNGS_ID:TOKEN https://example.com (auf macOS/Linux curl verwenden)",
  "faq.q15": "Was passiert mit meinen Sitzungen, wenn ich mich abmelde?",
  "faq.a15":
    "Beim Abmelden über die Kontoseite werden alle aktiven Käufer-Proxy-Sitzungen geschlossen und die Verkäuferverbindung gestoppt, falls sie läuft. Dein Sitzungstoken wird von der Festplatte gelöscht.",
  "faq.q16":
    "Wie verbinde ich mich für den Weiterverkauf über einen Upstream-Proxy?",
  "faq.a16":
    "Füge im Tab Verkäufer im Abschnitt „Upstream-Proxys' Upstream-Proxys hinzu. Jeder Eintrag hat Host:Port, Benutzername und Passwort. Der Verkäufer leitet Datenverkehr über diese Upstream-Proxys statt (oder zusätzlich) über deine eigene Verbindung. Aktiviere die Option „Direkt einbeziehen', um auch deine eigene Bandbreite zu verkaufen. Das Backend verteilt Streams per Round-Robin-Hash über alle Pfade.",
  "faq.q17": "Wofür ist Discord da?",
  "faq.a17":
    "Das Discord-Symbol im Kopfbereich verlinkt zum ProxyBase-Community-Server (discord.gg/7uedk7ajHD). Tritt bei für Support, Ankündigungen und Diskussionen mit anderen Verkäufern und Käufern.",
  "faq.q18": "Wo sehe ich meinen Verkäuferstatus und Systeminfos?",
  "faq.a18":
    "Alles auf der Kontoseite – tippe auf das Personensymbol oben rechts. Dort findest du deinen Verbindungsstatus, Wallet-Adresse und -Saldo, den Verkäuferstatus mit der Anzahl aktiver Streams, Systemdateipfade und die App-Version.",

  "layout.createDeposit": "Einzahlung erstellen",
  "layout.amount": "Betrag",
  "layout.other": "Andere",
  "layout.enterAmount": "Betrag eingeben...",
  "layout.currency": "Währung",
  "layout.creating": "Wird erstellt...",
  "layout.createDepositAction": "Einzahlung erstellen",
  "layout.cancel": "Abbrechen",
  "layout.enterValidAmount": "Gib einen gültigen Betrag ein",
  "layout.amountTooSmall":
    "Der Betrag ist zu klein. Das Minimum legt der Zahlungsanbieter fest. Versuche einen größeren Betrag.",

  "update.ready":
    "Update v{version} bereit. Neustart zum Übernehmen.",
  "update.downloading": "Update wird heruntergeladen ({percent}%)...",
  "update.available": "v{version} verfügbar",
  "update.restartFailed":
    "Neustart fehlgeschlagen – klicke erneut auf Neustart oder starte die App manuell neu.",
  "update.restart": "Neustart",
  "update.update": "Aktualisieren",

  "password.show": "ANZEIGEN",
  "password.hide": "AUSBLENDEN",

  "settings.title": "Einstellungen",
  "settings.desc": "App-Einstellungen und Sprache.",
  "settings.language": "Sprache",
  "settings.languageDesc":
    "Wähle die Sprache für die gesamte App. Standardmäßig folgt sie der Systemsprache.",
  "settings.followSystem": "Systemsprache verwenden",
  "settings.systemLangLabel": "System ({lang})",
};

export default de;
