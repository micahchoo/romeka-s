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
  'server.settingNotCount':
    '{key} సెట్టింగ్ తప్పనిసరిగా సున్నా కంటే ఎక్కువ ఉన్న పూర్ణాంకం లేదా null అయి ఉండాలి.',
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
  'server.importAdminsOnly':
    'కేవలం ఒక గ్లోబల్ లేదా సైట్ అడ్మినిస్ట్రేటర్ మాత్రమే ఆర్కైవ్‌ను ఇంపోర్ట్ చేయగలరు.',
  'server.importNotFound': '{id} ఐడీతో ఏ ఇంపోర్ట్ లేదు.',
  'server.importRequestUnreadable': 'అభ్యర్థనను చదవడం సాధ్యం కాదు: {reason}',
  'server.importPlanRefused': 'ఇంపోర్ట్ సెట్టింగ్‌లను సేవ్ చేయడం సాధ్యం కాదు: {reason}',
  'server.importStale':
    'మీరు దీన్ని తెరిచిన తర్వాత వేరే ఎవరైనా ఈ ఇంపోర్ట్‌ను సేవ్ చేశారు. వారి మార్పులను చూడటానికి దీన్ని రీలోడ్ చేయండి.',
  'server.importPlanFrozen':
    'ఈ ఇంపోర్ట్ ఇప్పటికే ప్రారంభమైంది, కాబట్టి దీని సెట్టింగ్‌లను మార్చడం సాధ్యం కాదు. దీన్ని మార్చడానికి ఒక కాపీని సృష్టించండి.',
  'server.importV1Plan':
    'ఈ ఇంపోర్ట్ పాత ఇంపోర్టర్ ద్వారా చేయబడింది, కాబట్టి దీన్ని ఇక్కడ తెరవడం లేదా కాపీ చేయడం సాధ్యం కాదు.',
  'server.importTypeExists':
    'Omeka లో "{label}" అనే పేరుతో ఒక ఐటమ్ రకం ఉంది. కొత్త రకం కోసం మరొక పేరును ఎంచుకోండి.',
  'server.importDeleteNotDraft':
    'ఇంకా జోడించని ఇంపోర్ట్‌ను మాత్రమే తొలగించవచ్చు. రన్ అయిన ఇంపోర్ట్ గత ఇంపోర్ట్‌లలో ఉంటుంది.',
  'server.importChanged':
    'చివరి తనిఖీ తర్వాత ఫైల్‌లు లేదా ఇంపోర్ట్ సెట్టింగ్‌లు మారాయి. మీరు దీన్ని రన్ చేసే ముందు ఇంపోర్ట్‌ను మళ్ళీ తనిఖీ చేయండి.',
  'server.importBlocked': 'తనిఖీ జాబితా చేసే సమస్యలు సరిచేసే వరకు ఈ ఇంపోర్ట్ రన్ చేయలేరు.',
  'server.importNothingToRun': 'ఈ ఇంపోర్ట్‌కు వ్రాయడానికి ఏమీ లేదు.',
  'server.importConfirmDelete':
    'ఈ ఇంపోర్ట్ {count} ఐటమ్‌లను, మీడియా మరియు ఐటమ్ సెట్‌లను తొలగిస్తుంది మరియు దీన్ని రద్దు చేయలేము. దీన్ని రన్ చేయడానికి ఆ సంఖ్యను నిర్ధారించండి.',
  'server.importBusy':
    'మరొక ఇంపోర్ట్ రన్ అవుతోంది లేదా రద్దు చేయబడుతోంది. అది ముగిసినప్పుడు మళ్ళీ ప్రయత్నించండి.',
  'server.importCannotRun':
    'ఇంకా జోడించని, మధ్యలో ఆగిపోయిన లేదా వైఫల్యాలతో ముగిసిన ఇంపోర్ట్‌ను మాత్రమే రన్ చేయవచ్చు.',
  'server.importCannotUndo':
    'కేవలం పూర్తయిన లేదా మధ్యలో ఆగిపోయిన ఇంపోర్ట్‌ను మాత్రమే రద్దు చేయవచ్చు.',
  'server.importNotRunning': 'ఈ ఇంపోర్ట్ రన్ కావడం లేదు.',
  'server.importNoRoot': '"{root}" పేరుతో ఎటువంటి ఆర్కైవ్ ఫోల్డర్ లేదు.',
  'server.importOutsideRoot': '"{path}" ఫోల్డర్ ఆర్కైవ్ ఫోల్డర్ వెలుపల ఉంది.',
  'server.importFolderMissing': 'ఆర్కైవ్‌లో "{path}" ఫోల్డర్ లేదు.',
  'server.importNotAFolder': '"{path}" ఒక ఫైల్, ఫోల్డర్ కాదు.',
  'server.importScanBusy':
    'ఈ ఇంపోర్ట్ రన్ అవుతోంది లేదా రద్దు చేయబడుతోంది. అది ముగిసిన తర్వాత మళ్ళీ ఫైళ్లను చూడండి.',
  'server.importNoLeftFile':
    'ఈ ఇంపోర్ట్‌కు ఇంకా వదిలివేయబడిన ఫైళ్ల జాబితా లేదు. ఒక రన్ ముగిసినప్పుడు ఇది వ్రాయబడుతుంది.',
  'server.importJobNotStarted': 'ఇంపోర్ట్ ప్రారంభం కాలేదు: {reason}',
  'server.importListColumnUnknown':
    'జాబితాలో "{column}" కాలమ్ ఉండకూడదు. ఒక ప్రాపర్టీ, ఐటమ్ సెట్స్, ఐటమ్ రకం లేదా విజిబిలిటీని ఎంచుకోండి.',
  'server.importListOneFile': 'ఫారమ్‌లో ఒక జాబితా ఫైల్‌ను పంపండి.',
  'server.importListType':
    '"{name}" అనేది .csv, .tsv, .xlsx, .xls లేదా .ods ఫైల్ కాదు. జాబితాను వీటిలో ఒకదానిగా సేవ్ చేసి, మళ్ళీ పంపండి.',
  'server.importListTooLarge': 'జాబితా 50 MB కంటే పెద్దది.',
  'server.importListUnread': 'జాబితాను చదవడం సాధ్యం కాలేదు: {reason}',
  'server.importListNoIdentifier':
    'జాబితాలో ఐడెంటిఫైయర్ కాలమ్ లేదు, కాబట్టి దాని వరుసలు వాటి ఐటమ్స్‌ను కనుగొనలేవు. మీరు డౌన్‌లోడ్ చేసిన జాబితా యొక్క ఐడెంటిఫైయర్ కాలమ్‌ను ఉంచండి.',
  'server.importPairsColumns':
    'జాబితాలో Row కాలమ్, Files కాలమ్ ఉండాలి. మీరు డౌన్‌లోడ్ చేసిన జాబితాలోని రెండు కాలమ్‌లను అలాగే ఉంచండి.',
  'server.importFileUnreadable': '"{path}" ఫైల్‌ను చదవడం సాధ్యం కాలేదు: {reason}',
  'server.importFileRefused': '"{path}" ఫైల్‌ను జోడించలేదు: {reason}',
  'server.importFileNotStored': '"{path}" ఫైల్‌ను నిల్వ చేయలేకపోయాము: {reason}',
  'server.importSourceRefused': '"{source}" జోడించలేదు: {reason}',
  'server.importMediaNoSource': 'ఈ మీడియాకు జోడించడానికి ఫైల్ గానీ చిరునామా గానీ పేర్కొనలేదు.',
  'server.importMediaNoItem': 'ఈ మీడియా యొక్క ఐటమ్ జోడించబడలేదు, కాబట్టి మీడియా కూడా జోడించబడలేదు.',
  'server.importResourceGone': 'ఈ రికార్డును మారుస్తున్న మీడియా, ఐటమ్ లేదా ఐటమ్ సెట్ Omekaలో లేదు.',
  'server.importRecordNotSaved': 'ఈ రికార్డును సేవ్ చేయలేకపోయాము: {reason}',
  'server.importLinksNotSaved': 'ఈ రికార్డు యొక్క లింక్‌లను భద్రపరచలేకపోయాము: {reason}',
  'server.importRecordNotDeleted': 'ఇది తొలగించబడలేదు: {reason}',
  'server.importDeleteNotUndone':
    'ఐటమ్‌లు, మీడియా లేదా ఐటమ్ సెట్‌లను తొలగించిన ఇంపోర్ట్‌ను రద్దు చేయడం సాధ్యం కాదు.',
  'server.importSitesNotJoined': 'ఈ ఐటమ్ జోడించబడింది, కానీ దాని సైట్‌లకు జోడించబడలేదు: {reason}',
  'server.importUndoEdited': 'దిగుమతి దీన్ని రాసిన తర్వాత ఇది సవరించబడింది.',
  'server.importUndoLinked':
    'ఈ దిగుమతికి వెలుపల ఉన్న ఒక ఐటమ్, మీడియా లేదా ఐటమ్ సెట్ దీనికి లింక్ అయి ఉంది.',
  'server.importUndoMediaAdded': 'ఈ దిగుమతికి వెలుపల ఈ ఐటమ్‌కు మీడియా జోడించబడింది.',
  'server.importUndoMembersAdded': 'ఈ ఇంపోర్ట్ వెలుపల ఉన్న ఐటమ్‌లు ఈ ఐటమ్ సెట్‌లో చేర్చబడ్డాయి.',
  'server.importUndoLaterImport': 'ఈ దిగుమతి తర్వాత దిగుమతి {import} దీన్ని మార్చింది.',
  'server.importUndoInUse':
    'ఈ దిగుమతికి వెలుపల ఉన్న ఐటమ్‌లు, మీడియా, ఐటమ్ సెట్‌లు లేదా ఐటమ్ రకాలు దీన్ని ఉపయోగిస్తున్నాయి.',
  'server.importUndoItemKept':
    'ఈ మీడియా యొక్క ఐటమ్ భద్రపరచబడుతుంది, కాబట్టి ఈ మీడియా కూడా భద్రపరచబడుతుంది.',
  'server.importUndoMemberKept':
    'ఈ ఐటమ్ సెట్ లోని ఒక ఐటమ్ భద్రపరచబడుతుంది, కాబట్టి ఈ ఐటమ్ సెట్ కూడా భద్రపరచబడుతుంది.',
  'server.importUndoStopped': 'రద్దు చేయడం ముగిసేలోపే ఆగిపోయింది: {reason}',
  'server.importUploadOwnerOnly':
    'ఈ దిగుమతిని ప్రారంభించిన వ్యక్తి, లేదా గ్లోబల్ అడ్మినిస్ట్రేటర్, మాత్రమే దీనికి ఫైల్‌లను జోడించగలరు.',
  'server.importUploadNotDraft':
    'ఫైల్‌లను ఈ దిగుమతి నడుస్తున్నప్పుడు మాత్రమే జోడించగలరు. మరిన్ని ఫైల్‌లను జోడించడానికి, ఈ దిగుమతి యొక్క కాపీని తయారు చేయండి.',
  'server.importUploadPath': '"{path}" ఫైల్ పేరును ఉపయోగించలేము: {reason}.',
  'server.importUploadTooLarge':
    'ఈ ఫైల్ {limit} బైట్ల కంటే పెద్దది, ఇది ఒక దిగుమతికి పట్టే గరిష్ట పరిమాణం.',
  'server.importUploadFull':
    'ఈ దిగుమతిలోని ఫైల్‌లు {limit} బైట్ల కంటే ఎక్కువ అవుతాయి, ఇది ఒక దిగుమతి గరిష్టంగా భరించగల పరిమాణం. కొన్ని ఫైల్‌లను తొలగించండి, లేదా ఇంకా దిగుమతి అవుతున్న ఫైల్‌లు పూర్తయ్యే వరకు వేచి ఉండి మళ్ళీ ప్రయత్నించండి.',
  'server.importUploadChunkTooLarge':
    'ఈ అభ్యర్థన {limit} బైట్ల కంటే ఎక్కువ డేటాను కలిగి ఉంది. ఫైల్‌ను చిన్న చిన్న భాగాలుగా పంపండి.',
  'server.importUploadInProgress':
    '"{path}" ఇంకా దిగుమతి అవుతోంది. మళ్ళీ ప్రారంభించే బదులు, ఆ ఫైల్‌ను అప్‌లోడ్ చేస్తూ కొనసాగించండి.',
  'server.importUploadNotFound': 'ఈ దిగుమతికి "{upload}" ఐడితో ఎటువంటి అప్‌లోడ్ లేదు.',
  'server.importUploadOffset':
    'సర్వర్‌కు ఈ ఫైల్ నుండి {received} బైట్లు అందాయి. వాటి తర్వాత వచ్చే బైట్లను పంపండి.',
  'server.importUploadPastLength': 'ఈ బైట్లు ఈ అప్‌లోడ్ ప్రకటించిన పరిమాణం కంటే ఎక్కువగా ఉన్నాయి.',
  'server.importUploadLength': 'ఫైల్‌ను బైట్లలో దాని పరిమాణంతో పాటు పంపండి.',
  'server.importUploadMetadata':
    'ఫైల్‌ను "filename" గా మరియు బేస్64లో దాని పేరుతో "name" గా పంపండి.',
  'server.importUploadOffsetMissing': 'అప్పటికే పంపిన బైట్ల సంఖ్యతో "Upload-Offset" పంపండి.',
  'server.importUploadContentType':
    'బైట్‌లను Content-Type application/offset+octet-stream తో పంపండి.',
  'server.importUploadVersion':
    'ఈ సర్వర్ రీజ్యూమబుల్ అప్‌లోడ్ ప్రోటోకॉल వెర్షన్ 1.0.0ని మాట్లాడుతుంది. Tus-Resumable: 1.0.0 పంపండి.',
  'server.importUploadInterrupted':
    'బైట్లు అభ్యర్థన ముగియకముందే రావడం ఆగిపోయాయి, కాబట్టి ఏవీ భద్రపరచబడలేదు. ఆఫ్‌సెట్ అడిగి, మళ్ళీ పంపండి.',
  'server.importZipEscapes':
    '"{name}" జిప్ ఫైల్‌లో దాని ఫోల్డర్ వెలుపల పడుతుంది, కాబట్టి దానిని వదిలివేశారు.',
  'server.importZipLink': '"{name}" జిప్ ఫైల్‌లో ఒక లింక్, కాబట్టి దానిని వదిలివేశారు.',
  'server.importZipUnusable':
    '"{name}" జిప్ ఫైల్‌లో ({reason}) ఉపయోగించలేని పేరును కలిగి ఉంది, కాబట్టి దానిని వదిలివేశారు.',
  'server.importZipUnsupported':
    '"{name}" జిప్ ఫైల్‌లో ఉంది, ఇది Romeka S విప్పలేని విధంగా ప్యాక్ చేయబడింది, కాబట్టి దానిని వదిలివేశారు.',
  'server.importZipRenamed':
    '"{name}" జిప్ ఫైల్‌లో ఉంది, ఇందులో పెద్ద అక్షరాలు (capitals) విస్మరించబడినప్పుడు మునుపటి ఫైల్ పేరు ఉంటుంది, కాబట్టి దానిని "{renamed}"గా జోడించారు.',
  'server.importZipBomb':
    'ZIP తిరస్కరించబడింది: "{name}" ప్యాక్ చేసిన పరిమాణం కంటే 100 రెట్లు ఎక్కువ అన్‌ప్యాక్ అవుతుంది. దీని నుండి ఏమీ జోడించబడలేదు.',
  'server.importZipFull':
    'ZIP తిరస్కరించబడింది: దాని ఫైల్‌లు {limit} బైట్ల కంటే ఎక్కువ ఉంటాయి, ఇది ఒక ఇంపోర్ట్ గరిష్టంగా కలిగి ఉండే పరిమాణం. దీని నుండి ఏమీ జోడించబడలేదు.',
  'server.importZipTooMany':
    'ZIP తిరస్కరించబట్టి: ఇందులో {count} ఫైల్‌లు ఉన్నాయి, ఇవి 100,000 కంటే ఎక్కువ. దీని నుండి ఏమీ జోడించబడలేదు.',
  'server.importZipFileTooLarge':
    'ZIP తిరస్కరించబట్టి: "{name}" {limit} బైట్ల కంటే పెద్దది, ఇది ఒక ఇంపోర్ట్ తీసుకునే అతిపెద్ద ఫైల్. దీని నుండి ఏమీ జోడించబడలేదు.',
  'server.importZipUnreadable':
    'ZIP తిరస్కరించబట్టి: దీనిని చదవడం సాధ్యం కాలేదు ({reason}). దీని నుండి ఏమీ జోడించబడలేదు.',
  'server.importZipUnpacked': 'ZIP విప్పబడింది: {count} ఫైల్‌లు జోడించబడ్డాయి.',
  'server.importSweepDone':
    '{imports} ఇంపోర్ట్‌ల ఫైల్‌లు తొలగించబడ్డాయి, మరియు {uploads} అసంపూర్ణ అప్‌లోడ్‌లు.',
  'server.importAccessFailed': '{media} మీడియా కోసం ఎటువంటి ప్లేయబుల్ కాపీ క్యూలో లేదు: {reason}',
  'server.importAccessUnread': 'ఈ ఇంపోర్ట్ కోసం ఎటువంటి ప్లేయబుల్ కాపీలు క్యూలో లేవు: {reason}',
  'server.mediaUploadNotFound': 'ఈ ఖాతాలో "{upload}" ఐడితో ఎటువంటి స్టేజ్డ్ అప్‌లోడ్ లేదు.',
  'server.mediaUploadBusy':
    'ఈ అప్‌లోడ్ "{upload}" ఇప్పుడే జోడించబడుతోంది. అది పూర్తయ్యే వరకు వేచి ఉండి మళ్ళీ ప్రయత్నించండి.',
  'server.mediaUploadIncomplete':
    'ఈ అప్‌లోడ్‌లో {received} బైట్లు {length} లో ఉన్నాయి. దీనిని జోడించే ముందు మిగిలిన వాటిని పంపండి.',
  'server.mediaUploadIngester': 'స్టేజ్డ్ అప్‌లోడ్ అప్‌లోడ్ ఇంజియర్‌తో జోడించబడింది.',
  'server.mediaUploadInline':
    'స్టేజ్డ్ అప్‌లోడ్ దాని స్వంత మీడియా క్రియేట్‌తో జోడించబడుతుంది, ఐటమ్ లోపల కాదు.',
  'server.uploadTooLarge':
    'ఈ ఫైల్ {limit} బైట్ల కంటే పెద్దది, ఇది ఒక మీడియా అప్‌లోడ్ తీసుకునే అతిపెద్ద ఫైల్.',
  'server.mediaLinkInvalid': '"{url}" లింక్ http లేదా https అడ్రస్ కాదు.',
  'server.mediaLinkUnreadable': '"{url}" లింక్‌ను చదవడం సాధ్యం కాలేదు: {reason}',
  'server.mediaLinkStatus': '"{url}" లింక్ {status} {reason} ఇచ్చింది.',
  'server.uploadItem': 'అప్‌లోడ్ యొక్క ఐటమ్ ఐడి, ఉనికిలో ఉన్న ఐటమ్‌ను సూచించాలి.',
  'server.mediaUploadSweepDone':
    '{count} స్టేజ్డ్ అప్‌లోడ్‌లు వాటి ఉంచిన సమయాన్ని దాటి తొలగించబడ్డాయి.',
};
