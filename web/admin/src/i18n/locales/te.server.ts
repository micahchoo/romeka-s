/**
 * The Telugu server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Telugu. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const teServer: ServerCatalogue = {
  'server.methodNotAllowed': 'పద్ధతి అనుమతించబడదు',
  'server.notFound': 'కనుగొనబడలేదు',
  'server.badRequest': 'తప్పుడు అభ్యర్థన',
  'server.requestTimeout': 'అభ్యర్థన సమయం ముగిసింది',
  'server.bodyTooLarge': 'అభ్యర్థన బాడీ చాలా పెద్దది',
  'server.internalError': 'అంతర్గత సర్వర్ లోపం',
  'server.tooManyRequests': 'చాలా అభ్యర్థనలు',
  'server.unknownResource': 'API, "{resource}" వనరును సమర్థించదు.',
  'server.adapterNotImplemented': '{resource} అడాప్టర్ {operation} ఆపరేషన్‌ను అమలు చేయదు.',
  'server.endpointNotImplemented': '{endpoint} ఎండ్‌పాయింట్ ఇంకా అమలు చేయబడలేదు.',
  'server.permissionDenied':
    'ప్రస్తుత వినియోగదారుకు {resource} వనరును {operation} చేయడానికి అనుమతి నిరాకరించబడింది.',
  'server.entityNotFound': '{class} ప్రమాణాలు {criteria} తో కనుగొనబడలేదు',
  'server.jsonSyntax': 'JSON: సింటాక్స్ లోపం',
  'server.jsonNotContainer': 'JSON: కంటెంట్ ఆబ్జెక్ట్ లేదా అర్రే అయి ఉండాలి.',
  'server.invalidContentType':
    'చెల్లని Content-Type హెడర్. "application/json" ఆశించబడింది, "{received}" వచ్చింది.',
  'server.bodyNotObject': 'అభ్యర్థన బాడీ JSON ఆబ్జెక్ట్ అయి ఉండాలి.',
  'server.siteItemsOneSelector': 'పేరు "query", లేదా "items", "item_sets" లేదా రెండూ ఉండాలి',
  'server.invalidCredentials': 'ఇమెయిల్ లేదా పాస్‌వర్డ్ చెల్లదు',
  'server.inactiveAccount': 'వినియోగదారు క్రియాశీలకంగా లేరు.',
  'server.unknownKey': 'చలని పాస్‌వర్డ్ సృష్టి కీ.',
  'server.authenticationRequired': 'ప్రామాణీకరణ అవసరం.',
  'server.invalidPassword': 'పాస్‌వర్డ్ చothing',
  'server.crossOriginRefused':
    'సెషన్-ధృవీకరించబడిన రైట్ తప్పనిసరిగా కాన్ఫిగర్ చేయబడిన బేస్ URL నుండి రావాలి.',
  'server.propertyNotRecognized': '"{term}" అనే ధర్మం గుర్తించబడలేదు.',
  'server.resourceNotFound': 'id {id} తో రిసోర్స్ కనుగొనబడలేదు.',
  'server.templateRequiresValue': '"{template}" రిసోర్స్ టెంప్లేట్‌కు "{property}" విలువ అవసరం',
  'server.itemSetsNotArray': 'ఐటమ్ సెట్లు తప్పనిసరిగా అర్రే అయి ఉండాలి',
  'server.mediaNotArray': 'మీడియా తప్పనిసరిగా అర్రే అయి ఉండాలి',
  'server.mediaNotThisItem': 'మీడియా ఈ ఐటమ్‌కు చెందాలి.',
  'server.primaryMediaNotThisItem': 'ప్రాథమిక మీడియా ఈ ఐటమ్‌కు చెందాలి.',
  'server.mediaIngesterRequired': 'మీడియా తప్పనిస ఇంగెస్టర్‌ను సెట్ చేయాలి.',
  'server.mediaIngesterInvalid': 'మీడియా తప్పనిస సరైన ఇంగెస్టర్‌ను సెట్ చేయాలి.',
  'server.mediaItemRequired': 'మీడియా ఒక ఐటమ్‌కు చెందాలి.',
  'server.jobHeldByImport':
    'ఈ జాబ్ ఒక ఆర్కైవ్ ఇంపోర్ట్ కి చెందినది, ఇది రన్ అవుతోంది లేదా అన్డూ చేస్తోంది. ఇంపోర్ట్ పూర్తయిన తర్వాత దానిని డిలీట్ చేయండి.',
  'server.pageSavedSinceOpened': 'మీరు ఈ పేజీని తెరిచిన తర్వాత ఎవరో ఒకరు దీనిని సేవ్ చేశారు.',
  'server.noFileUploaded': 'ఎటువంటి ఫైల్ అప్‌లోడ్ చేయబడలేదు',
  'server.assetMediaType':
    '"{file}"ను ధృవీకరించడంలో లోపం. "{media}" మీడియా రకంతో ఫైళ్లను నిల్వ చేయలేరు.',
  'server.assetExtension':
    '"{file}"ను ధృవీకరించడంలో లోపం. పరిష్కరించబడిన ఎక్స్‌టెన్షన్‌తో "{extension}" ఫైళ్లను నిల్వ చేయలేరు.',
  'server.assetNameRequired': 'అసెట్ తప్పనిస పేరు కలిగి ఉండాలి.',
  'server.classesNotArray': 'క్లాస్‌లు తప్పనిసగా అర్రే అయి ఉండాలి',
  'server.propertiesNotArray': 'ప్రాపర్టీలు తప్పనిసగా అర్రే అయి ఉండాలి',
  'server.namespaceUriEmpty': 'నేమ్‌స్పేస్ URI ఖాళీగా ఉండకూడదు.',
  'server.namespaceUriTaken': 'నేమ్‌స్పేస్ URI "{uri}" ఇప్పటికే ఉపయోగించబడింది.',
  'server.prefixEmpty': 'ప్రిఫిక్స్ ఖాళీగా ఉండకూడదు.',
  'server.prefixInvalid':
    'ప్రిఫిక్స్ కేవలం అక్షరాలు, సంఖ్యలు, హైఫన్లు మరియు అండర్‌స్కోర్‌లను మాత్రమే కలిగి ఉండవచ్చు.',
  'server.prefixTaken': 'ప్రిఫిక్స్ "{prefix}" ఇప్పటికే ఉపయోగించోంది.',
  'server.prefixReserved': 'ప్రిఫిక్స్ "{prefix}" రిజర్వ్ చేయబడింది.',
  'server.labelEmpty': 'లేబుల్ ఖాళీగా ఉండకూడదు.',
  'server.labelTaken': 'లేబుల్ ఇప్పటికే ఉపయోగించబడింది.',
  'server.localNameEmpty': 'స్థానిక పేరు ఖాళీగా ఉండకూడదు.',
  'server.localNameTaken': 'స్థానిక పేరు "{name}" ఇప్పటికే ఉపయోగించోంది.',
  'server.vocabularyRequired': 'పదజాలం తప్పనిసరిగా సెట్ చేయాలి.',
  'server.duplicateTemplateProperty':
    'ID {id} తో డూప్లికేట్ ప్రాపర్టీని జోడించడానికి ప్రయత్నిస్తోంది',
  'server.vocabularyNoFile': 'ఫైల్ ఇంపోర్ట్ స్ట్రాటజీ కోసం ఫైల్ పేర్కొనబడలేదు.',
  'server.vocabularyNoUrl': 'URL ఇంపోర్ట్ స్ట్రాటజీ కోసం URL పేర్కొనబడలేదు.',
  'server.vocabularyNotParsed': 'వొకాబులరీ ఫైల్ పార్స్ చేయలేకపోయాము.',
  'server.vocabularyNotLoaded': 'URL నుండి వొకాబులరీని లోడ్ చేయలేకపోయాము.',
  'server.vocabularyNoMembers':
    'క్లాస్‌లు లేదా ప్రాపర్టీలు కనుగొనబడలేదు. మీరు సరైన నేమ్‌స్పేస్ URIని ఉపయోగించారా?',
  'server.vocabularyPermanent': 'శాశ్వత వొకాబులరీని ఎడిట్ చేయలేరు',
  'server.vocabularyFormatUnknown':
    'ఫైల్ ఫార్మాట్‌ను గుర్తించలేకపోయాము. జాబితా నుండి ఫార్మాట్‌ను ఎంచుకోండి.',
  'server.vocabularyFormatInvalid': '"{format}" వొకాబులరీ ఫైల్ ఫార్మాట్ కాదు.',
  'server.vocabularyUrlScheme': 'కేవలం http మరియు https URLలను మాత్రమే ఇంపోర్ట్ చేయవచ్చు.',
  'server.vocabularyUrlPrivate':
    'URL ఒక ప్రైవేట్ లేదా లోకల్ నెట్‌వర్క్ అడ్రస్‌ను సూచిస్తుంది, దీనిని సర్వర్ పొందదు.',
  'server.vocabularyUrlTooLarge': 'URL వద్ద ఉన్న ఫైల్ {size} కంటే పెద్దది.',
  'server.vocabularyUrlTimeout': 'URL వద్ద ఉన్న సర్వర్ సమయానికి సమాధానం ఇవ్వలేదు.',
  'server.vocabularyUrlStatus': 'URL వద్ద ఉన్న సర్వర్ {status} స్టేటస్‌తో సమాధానం ఇచ్చింది.',
  'server.vocabularyChangeStale': '"{name}" పోలికలో మార్పు చెందింది. ఫైల్‌ను మళ్ళీ పోల్చండి.',
  'server.vocabularyMemberMissing': '"{name}" ఈ వొకాబులరీలో సభ్యుడు కాదు.',
  'server.nameEmpty': 'పేరు ఖాళీగా ఉండకూడదు.',
  'server.emailTaken': 'ఈమెయిల్ {email} ఇప్పటికే ఉపయోగించబడింది.',
  'server.userRoleRequired': 'వినియోగదారులకు పాత్ర ఉండాలి.',
  'server.invalidEmail':
    'ఇన్‌పుట్ సరైన ఇమెయిల్ అడ్రస్ కాదు. ప్రాథమిక ఫార్మాట్ local-part@hostname ఉపయోగించండి.',
  'server.siteTitleRequired': 'సైట్‌కు టైటిల్ ఉండాలి.',
  'server.siteThemeRequired': 'సైట్‌కు థీమ్ ఉండాలి.',
  'server.siteItemPoolRequired': 'సైట్‌కు ఐటమ్ పూల్ డేటా ఉండాలి.',
  'server.homepageNotInSite': 'హోమ్‌పేజీ దాని పేరెంట్ సైట్‌కు చెందాలి.',
  'server.slugEmpty': 'స్లగ్ ఖాళీగా ఉండకూడదు.',
  'server.slugInvalid':
    'స్లగ్‌లో కేవలం అక్షరాలు, సంఖ్యలు, అండర్‌స్కోర్‌లు మరియు హైఫన్‌లు మాత్రమే ఉండవచ్చు.',
  'server.slugTaken': 'స్లగ్ "{slug}" ఇప్పటికే ఉపయోగించబడింది.',
  'server.pageSiteRequired': 'పేజీ తప్పనిసరిగా సైట్‌కు చెందాలి.',
  'server.pageTitleRequired': 'పేజీకి టైటిల్ ఉండాలి.',
  'server.blockLayoutRequired': 'అన్ని బ్లాక్‌లకు లేఅవుట్ ఉండాలి.',
  'server.blockDataScalar': 'బ్లాక్ డేటా స్కేలార్ విలువ కాకూడదు.',
  'server.siteSettingsNotObject': 'ఆశించిన ఆబ్జెక్ట్ లేదా నల్, {kind} వచ్చింది.',
  'server.cssTooLarge':
    'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: కస్టమ్ CSS గరిష్టంగా {max} బైట్లు ఉండవచ్చు; ఇది {bytes}.',
  'server.cssImport': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: @import అనుమతించబడదు.',
  'server.cssDocument': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: @document అనుమతించబడదు.',
  'server.cssForeignUrl':
    'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: url({url}) అనుమతించబడదు; ఒక URL తప్పనిసరిగా /files/ లేదా /assets/ కింద పాత్ అయి ఉండాలి.',
  'server.cssExpression': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: expression() అనుమతించబడదు.',
  'server.cssBehavior': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: behavior అనుమతించబడదు.',
  'server.cssMozBinding': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: -moz-binding అనుమతించబడదు.',
  'server.cssStyleBreakout': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: </style అనుమతించబడదు.',
  'server.cssBadUrl': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: ఈ url( సరైన CSS కాదు.',
  'server.cssUnclosedString': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: ఈ స్ట్రింగ్ మూసివేయబడలేదు.',
  'server.cssStrayClose': 'కస్టమ్ CSS: లైన్ {line}, కాలమ్ {column}: {bracket} ఏమీ మూసివేయదు.',
  'server.navNotArray': 'చెల్లని నావిగేషన్: నావిగేషన్ తప్పనిసరిగా ఒక అర్రే అయి ఉండాలి',
  'server.navLinkMissingType': 'చGFSలని నావిగేషన్: లింక్ రకం లేదు',
  'server.navLinkMissingData': 'చGFSలని నావిగేషన్: లింక్ డేటా లేదు',
  'server.navInvalidLinkData': 'చGFSలని నావిగేషన్: చGFSలని లింక్ డేటా',
  'server.navPageLinksNotUnique': 'చGFSలని నావిగేషన్: పేజీ లింకులు ప్రత్యేకమై ఉండాలి',
  'server.navLinksNotArray': 'చGFSలని నావిగేషన్: లింకులు తప్పనిసరిగా ఒక అర్రే అయి ఉండాలి',
  'server.navPageLinkMissingPage': 'చGFSలని నావిగేషన్: పేజీ లింక్ పేజీ ID లేదు',
  'server.navUrlMissingLabel': 'చGFSలని నావిగేషన్: URL లింక్ లేబుల్ లేదు',
  'server.navUrlMissingUrl': 'చGFSలని నావిగేషన్: URL లింక్ URL లేదు',
  'server.navUrlInvalidCharacter': 'చGFSలని నావిగేషన్: URL లింక్ చGFSలని క్యారెక్టర్ కలిగి ఉంది',
  'server.navUrlInvalidScheme': 'చGFSలని నావిగేషన్: URL లింక్ చGFSలని స్కీమ్ కలిగి ఉంది',
  'server.settingsDenied':
    'ప్రస్తుత వినియోగదారుకు గ్లోబల్ సెట్టింగ్‌లను యాక్సెస్ చేయడానికి అనుమతి నిరాకరించబడింది.',
  'server.settingsNotObject': 'ఒక ఆబ్జెక్ట్ ఆశించబడింది.',
  'server.settingUnavailable': '{key} సెట్టింగ్ అందుబాటులో లేదు.',
  'server.settingNotText': '{key} సెట్టింగ్ స్ట్రింగ్ లేదా నల్ ఉండాలి.',
  'server.settingNotFlag': '{key} సెట్టింగ్ ట్రూ, ఫాల్స్ లేదా నల్ ఉండాలి.',
  'server.settingNotNames': '{key} సెట్టింగ్ స్ట్రింగ్స్ యొక్క జాబితా లేదా నల్ ఉండాలి.',
  'server.mediaNotPublic':
    'మీడియా {id} ప్రైవేట్‌గా ఉంది, కాబట్టి ఒక పేజీ దానిని చూపించలేదు. మీడియాను మరియు దాని ఐటమ్‌ను పబ్లిక్‌గా మార్చండి, లేదా మరొక మీడియాను ఎంచుకోండి.',
  'server.publicationTermsRemoved':
    'ఒక పబ్లిష్డ్ టర్మ్ తొలగించలేము లేదా రీనేమ్ చేయలేము: {terms}. దానికి బదులుగా డిప్రికేట్ చేయండి.',
  'server.publicationNamespaceFixed':
    'ఒక పబ్లిష్డ్ వోకాబులరీ తన నేమ్‌స్పేస్‌ను ఉంచుకుంటుంది, కాబట్టి దాని టర్మ్స్ వాటి URIsని ఉంచుకుంటాయి.',
  'server.publicationNotDeletable':
    'ఒక పబ్లిష్డ్ టర్మ్ లిస్ట్ లేదా వోకాబులరీని తొలగించలేము. దానికి బదులుగా దాని టర్మ్స్‌ను డిప్రికేట్ చేయండి.',
  'server.publicationDeprecationFixed':
    '"{term}" యొక్క డిప్రికేషన్ పబ్లిష్ చేయబడింది మరియు దానిని వెనక్కి తీసుకోలేము.',
  'server.publicationItemsList':
    'ఐటమ్స్ యొక్క లిస్ట్ పబ్లిష్ చేయలేము. ప్రతి ఐటమ్‌కు ఇప్పటికే దాని స్వంత అడ్రస్ ఉంది.',
  'server.publicationForeignNamespace':
    'నేమ్‌స్పేస్ {namespace} ఈ ఆర్కైవ్ యొక్కది కాదు. ఈ ఆర్కైవ్ యొక్క /vocab/ అడ్రస్ కింద ఉన్న నేమ్‌స్పేస్‌తో ఒక వోకాబులరీని పబ్లిష్ చేయండి, లేదా w3id.org లేదా purl.org లో.',
  'server.publicationSlugInvalid':
    'ఒక అడ్రస్ నేమ్ లో చిన్న అక్షరాలు, అంకెలు మరియు సింగిల్ హైఫన్‌లు, 64 క్యారెక్టర్ల వరకు ఉంటాయి.',
  'server.publicationSlugTaken': 'అడ్రస్ /vocab/{slug}/ ఇప్పటికే తీసుకోబడింది.',
  'server.publicationNamespaceNotPersistent':
    'ఈ ఆర్కైవ్ యొక్క ఇతర నేమ్‌స్పేస్ w3id.org లేదా purl.org లో ఉండాలి మరియు / లేదా # తో ముగియాలి.',
  'server.publicationAddressFixed':
    'ఒక పబ్లిష్డ్ టర్మ్ లిస్ట్ లేదా వోకాబులరీ అది మొదట పబ్లిష్ అయిన అడ్రస్‌ను ఉంచుకుంటుంది.',
  'server.publicationTermNotPublished':
    '"{term}" ఇంకా పబ్లిష్ కాలేదు, కాబట్టి దానిని డిప్రికేట్ చేయలేము. దానిని తొలగించండి.',
  'server.publicationUnknownTerm': '"{term}" ఈ లిస్ట్ లేదా వోకాబులరీకి చెందిన పదం కాదు.',
  'server.publicationMatchInvalid': 'ఒక మ్యాచ్ అబ్సల్యూట్ http లేదా https URI అయి ఉండాలి.',
  'server.publicationMatchTermLists':
    'కేవలం ఒక టర్మ్ లిస్ట్ లోని టర్మ్స్ మాత్రమే అథారిటీకి లింక్ అవుతాయి.',
  'server.publicationTermIriInvalid': 'టర్మ్ URI {uri} ఒక సరైన IRI కాదు.',
};
