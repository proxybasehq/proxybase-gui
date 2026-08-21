import type { Messages } from "../translations";

const it: Messages = {
  "common.loading": "Caricamento...",
  "common.back": "Indietro",
  "common.close": "Chiudi",
  "common.cancel": "Annulla",
  "common.refresh": "Aggiorna",
  "common.create": "Crea",
  "common.creating": "Creazione...",
  "common.copied": "Copiato!",
  "common.address": "Indirizzo",
  "common.currency": "Valuta",
  "common.username": "Nome utente",
  "common.password": "Password",
  "common.status": "Stato",
  "common.country": "Paese",
  "common.type": "Tipo",
  "common.amount": "Importo",
  "common.login": "Accedi",
  "common.running": "In esecuzione",
  "common.reconnecting": "Riconnessione...",
  "common.stopped": "Arrestato",
  "common.support": "Supporto",
  "common.direct": "Diretto",
  "common.other": "Altro",
  "common.continue": "Continua",

  "nav.market": "Mercato",
  "nav.seller": "Venditore",
  "nav.faq": "FAQ",
  "nav.login": "Accedi",
  "nav.wallet": "Portafoglio",
  "nav.settings": "Impostazioni",
  "nav.account": "Account",
  "nav.discord": "Discord",

  "status.authenticated": "Autenticato",
  "status.notAuthenticated": "Non autenticato",

  "welcome.title": "Benvenuto su ProxyBase",
  "welcome.subtitle":
    "Un marketplace decentralizzato di banda tra pari. Compra e vendi accesso proxy usando depositi in criptovaluta.",
  "welcome.createWallet": "Crea nuovo portafoglio",
  "welcome.importWallet": "Importa portafoglio esistente",
  "welcome.createTitle": "Crea portafoglio",
  "welcome.createDesc":
    "Genera un nuovo portafoglio BIP-39. Conserva la tua frase mnemonica in modo sicuro.",
  "welcome.encryptionPassword": "Password di cifratura (opzionale)",
  "welcome.passwordPlaceholder": "Lascia vuoto per nessuna password",
  "welcome.walletCreated": "Portafoglio creato",
  "welcome.saveMnemonic": "Salva la tua frase mnemonica",
  "welcome.saveMnemonicDesc":
    "Scrivi queste 12 parole in ordine. Chiunque abbia questa frase può accedere al tuo portafoglio. Non condividerla mai.",
  "welcome.signingIn": "Accesso in corso...",
  "welcome.continue": "Continua",
  "welcome.importTitle": "Importa portafoglio",
  "welcome.importDesc":
    "Ripristina il tuo portafoglio da una frase mnemonica BIP-39.",
  "welcome.mnemonicPhrase": "Frase mnemonica di 12 parole",
  "welcome.mnemonicPlaceholder":
    "Inserisci le 12 parole separate da spazi...",
  "welcome.importing": "Importazione...",
  "welcome.importAndLogin": "Importa e accedi",
  "welcome.autoLoginFailed":
    "Accesso automatico fallito. Crea o importa un portafoglio.",
  "welcome.enterMnemonic": "Inserisci la tua frase mnemonica",

  "login.title": "Accedi",
  "login.desc":
    "Autenticati con il tuo portafoglio per accedere alla rete ProxyBase.",
  "login.noWallet": "Nessun portafoglio trovato",
  "login.goToWallet": "Vai al portafoglio",
  "login.noWalletDesc":
    "Crea o importa prima un portafoglio dalla pagina Portafoglio.",
  "login.authenticate": "Autentica",
  "login.walletAddress": "Indirizzo del portafoglio",
  "login.walletPassword": "Password del portafoglio",
  "login.passwordPlaceholder":
    "Inserisci la password di cifratura del portafoglio (lascia vuoto se assente)",
  "login.authenticating": "Autenticazione...",
  "login.successful": "Accesso riuscito",
  "login.role": "Ruolo:",
  "login.buyerAvailable": "Disponibile come acquirente:",
  "login.spendableBalance": "Saldo spendibile:",

  "wallet.title": "Portafoglio",
  "wallet.desc": "Gestisci la tua identità di portafoglio ProxyBase.",
  "wallet.info": "Info",
  "wallet.create": "Crea",
  "wallet.import": "Importa",
  "wallet.status": "Stato del portafoglio",
  "wallet.addressLabel": "Indirizzo:",
  "wallet.loaded": "Caricato",
  "wallet.noWallet":
    "Nessun portafoglio trovato. Creane uno o importane uno.",
  "wallet.clickInfo":
    "Fai clic su Info per controllare lo stato del portafoglio.",
  "wallet.createNew": "Crea nuovo portafoglio",
  "wallet.encryptionPassword": "Password di cifratura (opzionale)",
  "wallet.passwordPlaceholder": "Lascia vuoto per nessuna password",
  "wallet.createWallet": "Crea portafoglio",
  "wallet.creating": "Creazione...",
  "wallet.walletAddress": "Indirizzo del portafoglio",
  "wallet.mnemonic": "Frase mnemonica — SALVALA IN MODO SICURO",
  "wallet.mnemonicWarning":
    "Scrivi queste 12 parole in ordine. Chiunque abbia questa frase può accedere al tuo portafoglio.",
  "wallet.importFromMnemonic": "Importa da frase mnemonica",
  "wallet.mnemonicPhrase": "Frase mnemonica di 12 parole",
  "wallet.mnemonicPlaceholder":
    "Inserisci le 12 parole separate da spazi...",
  "wallet.importing": "Importazione...",
  "wallet.importWallet": "Importa portafoglio",
  "wallet.imported": "Importato",

  "market.title": "Mercato",
  "market.desc":
    "Esplora paesi, prezzi e gestisci le sessioni proxy.",
  "market.prices": "Prezzi",
  "market.activeSessions": "Sessioni attive",
  "market.insufficientBalance": "Saldo insufficiente",
  "market.insufficientDesc":
    "Non hai fondi sufficienti. Deposita criptovaluta per continuare.",
  "market.depositFunds": "Deposita fondi",
  "market.dismiss": "Ignora",
  "market.connectionDetails": "Dettagli connessione proxy",
  "market.remote": "Remoto",
  "market.localBridge": "Bridge locale",
  "market.proxyAddress": "Indirizzo proxy",
  "market.sessionId": "ID sessione",
  "market.password": "Password",
  "market.country": "Paese",
  "market.type": "Tipo",
  "market.exampleCurl": "Esempio (curl)",
  "market.localBridgeDesc":
    "Usa il bridge locale per app come Chrome che non supportano proxy autenticati.",
  "market.auth": "Autenticazione",
  "market.noneRequired": "Non richiesta",
  "market.exampleCurlLocal": "Esempio (curl • locale)",
  "market.bridgeNotRunning":
    "Bridge non attivo. La sessione potrebbe essere stata acquistata da un altro dispositivo.",
  "market.pricing": "Prezzi",
  "market.refresh": "Aggiorna",
  "market.loadingPricing": "Caricamento dati prezzi...",
  "market.category": "Categoria",
  "market.price": "Prezzo",
  "market.buying": "Acquisto...",
  "market.buy": "Acquista",
  "market.noSellers": "Nessun venditore disponibile.",
  "market.activeSessionsCount": "Sessioni attive ({count})",
  "market.noActiveSessions":
    "Nessuna sessione attiva. Acquistane una nella scheda Prezzi.",
  "market.mode": "Modalità",
  "market.sessionType": "Tipo di sessione",
  "market.rotating": "Rotante",
  "market.sticky": "Persistente (10m)",
  "market.rotatingDesc": "Nuovo IP per richiesta",
  "market.stickyDesc": "Stesso IP fino a 10 min",
  "market.closeSession": "Chiudi sessione",

  "seller.title": "Venditore",
  "seller.desc":
    "Inizia a vendere la tua banda sul marketplace ProxyBase.",
  "seller.status": "Stato del venditore",
  "seller.refreshStatus": "Aggiorna stato",
  "seller.startStop": "Avvia / ferma venditore",
  "seller.includeDirect": "Includi diretto (vendi la tua banda)",
  "seller.volunteerMode": "Modalità volontario (dona larghezza di banda alla rete senza guadagni)",
  "seller.upstreamProxies": "Proxy a monte (rivendita)",
  "seller.hostPort": "Host:Porta",
  "seller.username": "Nome utente",
  "seller.password": "Password",
  "seller.remove": "Rimuovi",
  "seller.addUpstream": "+ Aggiungi proxy a monte",
  "seller.startSeller": "Avvia venditore",
  "seller.stopSeller": "Ferma venditore",
  "seller.activeStreams": "Flussi attivi ({count})",
  "seller.noStreams": "Nessun flusso attivo. In attesa di connessioni...",
  "seller.sessionId": "ID sessione",
  "seller.target": "Destinazione",
  "seller.route": "Percorso",
  "seller.proxyN": "Proxy #{n}",

  "deposit.title": "Depositi",
  "deposit.desc": "Crea depositi e controlla il loro stato.",
  "deposit.create": "Crea deposito",
  "deposit.amountUsd": "Importo ($USD)",
  "deposit.currency": "Valuta",
  "deposit.createAction": "Crea",
  "deposit.creating": "Creazione...",
  "deposit.checkStatus": "Controlla stato deposito",
  "deposit.depositId": "ID deposito",
  "deposit.enterDepositId": "Inserisci l'ID del deposito...",
  "deposit.checkStatusAction": "Controlla stato",
  "deposit.invalidAmount": "Importo non valido",

  "depositPage.created":
    "Deposito creato — invia esattamente l'importo mostrato",
  "depositPage.paymentQr": "QR di pagamento",
  "depositPage.address": "Indirizzo",
  "depositPage.currency": "Valuta",
  "depositPage.amount": "Importo",
  "depositPage.depositId": "ID deposito",
  "depositPage.timeRemaining": "tempo rimanente",
  "depositPage.complete": "Deposito completato",
  "depositPage.status": "Stato",
  "depositPage.credited": "Accreditato",
  "depositPage.done": "Fine",
  "depositPage.expired":
    "Il tempo di pagamento è scaduto. Crea un nuovo deposito.",
  "depositPage.tryAgain": "Riprova",
  "depositPage.close": "Chiudi",

  "account.wallet": "Portafoglio",
  "account.address": "Indirizzo",
  "account.viewBalance": "Visualizza saldo",
  "account.manageWallet": "Gestisci portafoglio",
  "account.addFunds": "Aggiungi fondi",
  "account.noWallet": "Nessun portafoglio caricato.",
  "account.seller": "Venditore",
  "account.streamsOne": "{count} flusso attivo",
  "account.streamsOther": "{count} flussi attivi",
  "account.sellerSettings": "Impostazioni venditore",
  "account.system": "Sistema",
  "account.dataDir": "Cartella dati",
  "account.walletPath": "Portafoglio",
  "account.sessionPath": "Sessione",
  "account.configPath": "Configurazione",
  "account.version": "Versione",
  "account.logout": "Esci",
  "account.checkUpdate": "Controlla aggiornamenti",
  "account.checkingUpdate": "Controllo aggiornamenti…",
  "account.upToDate": "Stai usando l'ultima versione",
  "account.updateAvailable": "Nuova versione v{version} disponibile. Installare e riavviare?",
  "account.installUpdate": "Installa",
  "account.downloadingUpdate": "Download dell'aggiornamento…",
  "account.updateError": "Controllo aggiornamenti non riuscito",
  "account.logoutWarning":
    "Tutte le sessioni attive verranno chiuse e il venditore verrà fermato.",
  "account.walletBalance": "Saldo del portafoglio",
  "account.failedBalance": "Impossibile caricare il saldo.",
  "account.appInfo": "Info app",
  "account.spendable": "Spendibile",
  "account.buyerAvailable": "Disponibile acquirente",
  "account.buyerReserved": "Riservato acquirente",
  "account.buyerSpent": "Speso acquirente",
  "account.sellerPending": "In attesa venditore",
  "account.sellerAvailable": "Disponibile venditore",
  "account.payoutLocked": "Pagamento bloccato",

  "faq.title": "FAQ",
  "faq.desc": "Domande frequenti su ProxyBase Markets.",
  "faq.q1": "Cos'è ProxyBase Markets?",
  "faq.a1":
    "ProxyBase Markets è un marketplace decentralizzato di banda tra pari. I venditori offrono le loro connessioni Internet come uscite proxy e gli acquirenti acquistano l'accesso a questi proxy per web scraping, navigazione di agenti IA e altro traffico automatizzato. Tutti i pagamenti sono regolati in microcrediti coperti da depositi in criptovaluta.",
  "faq.q2": "Come inizio a vendere la mia banda?",
  "faq.a2":
    "Vai alla scheda Venditore, configura i proxy a monte (facoltativi) e fai clic su 'Avvia venditore'. Il tuo nodo si registra sul marketplace e inizia a ricevere sonde QoS. Dopo aver superato i controlli di qualità, la tua connessione viene classificata per paese e tipo di rete e diventa acquistabile dagli acquirenti. Guadagni crediti per ogni GB di traffico inoltrato.",
  "faq.q3": "Quali sono i tipi di rete?",
  "faq.a3":
    "ProxyBase classifica le connessioni dei venditori in cinque tipi: Residenziale (ISP domestico), Mobile (operatore cellulare), Datacenter (IP cloud/colo), ISP (IP aziendali/statiche) e Burner (IP VPN/tor/proxy). Le IP Burner vengono segnalate durante le sonde QoS e possono ricevere prezzi più bassi o essere limitate per alcuni gruppi di acquirenti.",
  "faq.q4": "Come acquisto una sessione proxy?",
  "faq.a4":
    "Vai alla scheda Mercato → Prezzi. Trova un paese e un tipo di rete con venditori disponibili, poi fai clic sul pulsante verde 'Acquista' di quella riga. Viene creata una sessione proxy SOCKS5 che appare sotto la scheda 'Sessioni attive'. Il tuo saldo viene addebitato per GB di traffico utilizzato. Chiudi una sessione in qualsiasi momento con il pulsante X della sua riga.",
  "faq.q5": "Come funzionano i microcrediti e i prezzi?",
  "faq.a5":
    "1.000.000 di microcrediti = 1,00 USD. Il prezzo è fissato per paese e tipo di rete (es. 0,50 $/GB per residenziale USA). I venditori guadagnano crediti quando gli acquirenti usano i loro proxy. Puoi depositare fondi con criptovalute (BTC, USDC, USDT, SOL, ecc.) tramite NOWPayments.",
  "faq.q6": "Cos'è un deposito e come ne creo uno?",
  "faq.a6":
    "Apri la pagina Account dall'icona nell'intestazione, poi tocca 'Aggiungi fondi'. Scegli un importo (10, 20, 100 $ o personalizzato) e una criptovaluta. Dopo la creazione, verrai portato a una pagina di deposito con l'indirizzo di pagamento, il codice QR e un conto alla rovescia di 9 minuti. Invia esattamente l'importo mostrato a quell'indirizzo: il tuo saldo si aggiorna automaticamente una volta confermato.",
  "faq.q7": "Come controllo il saldo del mio portafoglio?",
  "faq.a7":
    "Vai alla pagina Account e tocca 'Visualizza saldo'. Vengono mostrati i tuoi saldi, inclusi saldo spendibile, disponibile/riservato/speso come acquirente e guadagni in attesa/disponibili/bloccati come venditore.",
  "faq.q8": "Cosa succede quando chiudo l'app?",
  "faq.a8":
    "L'app continua a essere eseguita nella barra di sistema. Chiudere la finestra la nasconde — non la chiude. Fai clic sull'icona nella barra per mostrarla o nasconderla. Se stavi vendendo banda, la tua sessione di venditore persiste e si riavvia automaticamente al prossimo avvio. L'app si registra anche per l'avvio automatico all'accensione del sistema.",
  "faq.q9": "Come creo un portafoglio?",
  "faq.a9":
    "La schermata di benvenuto ti guida nella creazione del portafoglio al primo avvio. Per gestirlo in seguito, vai alla pagina Account → 'Gestisci portafoglio'. Usa la scheda 'Crea' per generare una nuova frase mnemonica BIP-39 (12 parole), oppure 'Importa' per ripristinarla da una frase esistente. Conserva le parole in modo sicuro: sono l'unico modo per recuperare il portafoglio. I tuoi dati sono memorizzati in ~/.proxybase/.",
  "faq.q10": "L'app esegue l'accesso automaticamente?",
  "faq.a10":
    "Sì. All'avvio la schermata di benvenuto rileva il tuo portafoglio esistente e accede automaticamente se non è impostata una password. Se hai impostato una password del portafoglio, dovrai inserirla nella pagina di accesso.",
  "faq.q11": "Quali dati vengono memorizzati sul mio computer?",
  "faq.a11":
    "Tutto è memorizzato in ~/.proxybase/: il file di chiavi cifrato del portafoglio (wallet/keyfile.enc), il token di sessione (session_token) e la configurazione (config.toml). Le chiavi private non vengono mai inviate al backend — l'autenticazione usa firme crittografiche del tuo portafoglio locale.",
  "faq.q12":
    "Cosa sono le sonde QoS e come influenzano il mio stato di venditore?",
  "faq.a12":
    "Le sonde di qualità del servizio sono test automatizzati eseguiti dal marketplace per verificare velocità, latenza e affidabilità della tua connessione. Le sonde si connettono attraverso il tuo relay, misurano latenza e disponibilità e classificano la tua IP (paese, ISP, tipo di rete). I venditori che falliscono le sonde vengono sospesi. Una buona performance costante porta alla promozione dal pool Trial alla Produzione.",
  "faq.q13": "Quali sono i livelli del pool dei venditori?",
  "faq.a13":
    "Trial: nuovi venditori in valutazione QoS. Produzione: venditori verificati con affidabilità dimostrata — prioritari per le sessioni degli acquirenti. Sospeso: venditori che hanno fallito le sonde o sono andati offline — rivalutati automaticamente dopo un periodo di blocco.",
  "faq.q14":
    "Come uso il proxy SOCKS5 dopo l'acquisto di una sessione?",
  "faq.a14":
    "Fai clic su una riga di sessione nella scheda Sessioni attive per vedere i dettagli di connessione. Configura la tua applicazione per usare un proxy SOCKS5 su {proxyAddress} con l'ID sessione come nome utente e il tuo token di sessione come password. Esempio: curl.exe --socks5 {proxyAddress} --proxy-user ID_SESSIONE:TOKEN https://example.com (usa curl su macOS/Linux)",
  "faq.q15": "Cosa succede alle mie sessioni quando esco?",
  "faq.a15":
    "Uscire dalla pagina Account chiude tutte le sessioni proxy attive dell'acquirente e ferma la connessione del venditore se è in esecuzione. Il tuo token di sessione viene eliminato dal disco.",
  "faq.q16":
    "Come mi collego tramite un proxy a monte per la rivendita?",
  "faq.a16":
    "Nella scheda Venditore, aggiungi proxy a monte nella sezione 'Proxy a monte'. Ogni voce ha host:porta, nome utente e password. Il venditore inoltra il traffico attraverso questi proxy a monte invece (o in aggiunta) della tua connessione. Usa la casella 'Includi diretto' per vendere anche la tua banda. Il backend distribuisce i flussi su tutti i percorsi usando un hash round-robin.",
  "faq.q17": "A cosa serve Discord?",
  "faq.a17":
    "L'icona Discord nell'intestazione collega al server della community ProxyBase (discord.gg/7uedk7ajHD). Unisciti per supporto, annunci e discussioni con altri venditori e acquirenti.",
  "faq.q18":
    "Dove posso vedere il mio stato di venditore e le info di sistema?",
  "faq.a18":
    "Tutto è nella pagina Account — tocca l'icona della persona in alto a destra. Troverai lo stato della connessione, indirizzo e saldo del portafoglio, stato del venditore con numero di flussi attivi, percorsi dei file di sistema e versione dell'app.",

  "layout.createDeposit": "Crea deposito",
  "layout.amount": "Importo",
  "layout.other": "Altro",
  "layout.enterAmount": "Inserisci un importo...",
  "layout.currency": "Valuta",
  "layout.creating": "Creazione...",
  "layout.createDepositAction": "Crea deposito",
  "layout.cancel": "Annulla",
  "layout.enterValidAmount": "Inserisci un importo valido",
  "layout.amountTooSmall":
    "L'importo è troppo piccolo. Il minimo è stabilito dal fornitore di pagamenti. Prova con un importo maggiore.",

  "update.ready": "Aggiornamento v{version} pronto. Riavvia per applicarlo.",
  "update.downloading": "Download dell'aggiornamento ({percent}%)...",
  "update.available": "v{version} disponibile",
  "update.restartFailed":
    "Riavvio non riuscito — fai clic di nuovo su Riavvia o rilancia manualmente.",
  "update.restart": "Riavvia",
  "update.update": "Aggiorna",

  "password.show": "MOSTRA",
  "password.hide": "NASCONDI",

  "settings.title": "Impostazioni",
  "settings.desc": "Preferenze e lingua dell'app.",
  "settings.language": "Lingua",
  "settings.languageDesc":
    "Scegli la lingua usata in tutta l'app. Per impostazione predefinita segue la lingua di sistema.",
  "settings.followSystem": "Segui la lingua di sistema",
  "settings.systemLangLabel": "Sistema ({lang})",
};

export default it;
