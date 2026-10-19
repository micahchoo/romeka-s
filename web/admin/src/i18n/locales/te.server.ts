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
  'server.siteItemsOneSelector':
    'పేరు ఖచ్చితంగా "query", "items" లేదా "item_sets" లలో ఒకటి అయి ఉండాలి',
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
};
