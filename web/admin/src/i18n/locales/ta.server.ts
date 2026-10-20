/**
 * The Tamil server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const taServer: ServerCatalogue = {
  'server.methodNotAllowed': 'முறை அனுமதிக்கப்படவில்லை',
  'server.notFound': 'கண்டறியப்படவில்லை',
  'server.badRequest': 'தவறான கோரிக்கை',
  'server.requestTimeout': 'கோரிக்கை காலாவதி',
  'server.bodyTooLarge': 'கோரிக்கை உடல் மிகப் பெரியது',
  'server.internalError': 'உள் சேவையகப் பிழை',
  'server.tooManyRequests': 'அதிகப்படியான கோரிக்கைகள்',
  'server.unknownResource': 'API ஆனது "{resource}" வளத்தை ஆதரிக்கவில்லை.',
  'server.adapterNotImplemented':
    '{resource} அடாப்டர் {operation} செயல்பாட்டைச் செயல்படுத்தவில்லை.',
  'server.endpointNotImplemented': '{endpoint} இறுதிப்புள்ளி இன்னும் செயல்படுத்தப்படவில்லை.',
  'server.permissionDenied':
    'தற்போதைய பயனர் {operation} {resource} வளத்தை அணுக அனுமதி மறுக்கப்பட்டுள்ளது.',
  'server.entityNotFound': '{class} நிறுவனம் {criteria} அளவுகோல்களுடன் காணப்படவில்லை',
  'server.jsonSyntax': 'JSON: தொடரியல் பிழை',
  'server.jsonNotContainer': 'JSON: உள்ளடக்கம் ஒரு பொருளாகவோ அல்லது வரிசையாகவோ இருக்க வேண்டும்.',
  'server.invalidContentType':
    'தவறான Content-Type தலைப்பு. "application/json" எதிர்பார்க்கப்பட்டது, ஆனால் "{received}" கிடைத்தது.',
  'server.bodyNotObject': 'கோரிக்கை உடல் ஒரு JSON பொருளாக இருக்க வேண்டும்.',
  'server.siteItemsOneSelector':
    '"query", "items" அல்லது "item_sets" ஆகியவற்றில் சரியாக ஒன்றைக் குறிப்பிடவும்',
  'server.invalidCredentials': 'மின்னஞ்சல் அல்லது கடவுச்சொல் தவறானது',
  'server.inactiveAccount': 'பயனர் செயல்பாட்டில் இல்லை.',
  'server.unknownKey': 'தவறான கடவுச்சொல் உருவாக்கும் சாவி.',
  'server.authenticationRequired': 'அங்கீகாரம் தேவை.',
  'server.invalidPassword': 'கடவுச்சொல் தவறானது',
  'server.crossOriginRefused':
    'ஒரு அமர்வு-அங்கீகரிக்கப்பட்ட எழுதும் செயல்பாடு கட்டமைக்கப்பட்ட அடிப்படை URL-லிருந்து வர வேண்டும்.',
  'server.propertyNotRecognized': '"{term}" என்ற பண்பு அங்கீகரிக்கப்படவில்லை.',
  'server.resourceNotFound': '{id} என்ற அடையாளத்துடன் எந்த வளமும் கண்டறியப்படவில்லை.',
  'server.templateRequiresValue': '"{template}" வள வார்ப்புருவிற்கு ஒரு "{property}" மதிப்பு தேவை',
  'server.itemSetsNotArray': 'உருப்படித் தொகுப்புகள் ஒரு வரிசையாக (array) இருக்க வேண்டும்',
  'server.mediaNotArray': 'ஊடகங்கள் ஒரு வரிசையாக இருக்க வேண்டும்',
  'server.mediaNotThisItem': 'ஊடகங்கள் இந்த உருப்படியைச் சார்ந்திருக்க வேண்டும்.',
  'server.primaryMediaNotThisItem': 'முதன்மை ஊடகங்கள் இந்த உருப்படியைச் சார்ந்திருக்க வேண்டும்.',
  'server.mediaIngesterRequired': 'ஊடகங்கள் ஒரு உள்ளீட்டாளரை (ingester) அமைக்க வேண்டும்.',
  'server.mediaIngesterInvalid': 'ஊடகங்கள் ஒரு சரியான உள்ளீட்டாளரை அமைக்க வேண்டும்.',
  'server.mediaItemRequired': 'ஊடகங்கள் ஒரு உருப்படியைச் சார்ந்திருக்க வேண்டும்.',
  'server.jobHeldByImport':
    'இந்த வேலை ஒரு ஆவணக் காப்பக இறக்குமதிக்குச் சொந்தமானது, இது இயங்குகிறது அல்லது ரத்து செய்யப்படுகிறது. இறக்குமதி முடிந்ததும் அதை நீக்கவும்.',
  'server.noFileUploaded': 'கோப்பு எதுவும் பதிவேற்றப்படவில்லை',
  'server.assetMediaType':
    '"{file}" ஐச் சரிபார்ப்பதில் பிழை. "{media}" ஊடக வகையுடன் கோப்புகளைச் சேமிக்க முடியாது.',
  'server.assetExtension':
    '"{file}" ஐச் சரிபார்ப்பதில் பிழை. தீர்க்கப்பட்ட நீட்டிப்பு "{extension}" உடன் கோப்புகளைச் சேமிக்க முடியாது.',
  'server.assetNameRequired': 'ஒரு சொத்து (asset) பெயரைக் கொண்டிருக்க வேண்டும்.',
  'server.classesNotArray': 'வகுப்புகள் ஒரு வரிசையாக (array) இருக்க வேண்டும்',
  'server.propertiesNotArray': 'பண்புகள் ஒரு வரிசையாக இருக்க வேண்டும்',
  'server.namespaceUriEmpty': 'பெயர்வெளி URI காலியாக இருக்கக்கூடாது.',
  'server.namespaceUriTaken': 'பெயர்வெளி URI "{uri}" ஏற்கனவே பயன்படுத்தப்பட்டுள்ளது.',
  'server.prefixEmpty': 'முன்னொட்டு (prefix) காலியாக இருக்கக்கூடாது.',
  'server.prefixInvalid':
    'முன்னொட்டு எழுத்துக்கள், எண்கள், ஹைபன்கள் மற்றும் அண்டர்ஸ்கோர்களை மட்டுமே கொண்டிருக்கலாம்.',
  'server.prefixTaken': 'முன்னொட்டு "{prefix}" ஏற்கனவே பயன்படுத்தப்பட்டுள்ளது.',
  'server.prefixReserved': 'முன்னொட்டு "{prefix}" ஒதுக்கப்பட்டுள்ளது.',
  'server.labelEmpty': 'லேபிள் காலியாக இருக்கக்கூடாது.',
  'server.labelTaken': 'லேபிள் ஏற்கனவே பயன்படுத்தப்பட்டுள்ளது.',
  'server.localNameEmpty': 'உள்ளூர் பெயர் காலியாக இருக்கக்கூடாது.',
  'server.localNameTaken': 'உள்ளூர் பெயர் "{name}" ஏற்கனவே பயன்படுத்தப்பட்டுள்ளது.',
  'server.vocabularyRequired': 'ஒரு சொல்லகராதி அமைக்கப்பட வேண்டும்.',
  'server.duplicateTemplateProperty': 'ஐடி {id} உடன் நகல் பண்பைச் சேர்க்க முயற்சித்தல்',
  'server.vocabularyNoFile': 'கோப்பு இறக்குமதி உத்திக்காக எந்தக் கோப்பும் குறிப்பிடப்படவில்லை.',
  'server.vocabularyNoUrl': 'URL இறக்குமதி உத்திக்காக எந்த URL-உம் குறிப்பிடப்படவில்லை.',
  'server.vocabularyNotParsed': 'சொல்லகராதி கோப்பைப் பகுப்பாய்வு செய்ய முடியவில்லை.',
  'server.vocabularyNotLoaded': 'URL-லிருந்து சொல்லகராதியை ஏற்ற முடியவில்லை.',
  'server.vocabularyNoMembers':
    'வகுப்புகள் அல்லது பண்புகள் எதுவும் காணப்படவில்லை. சரியான பெயர்வெளி URI-ஐப் பயன்படுத்தியுள்ளீர்களா?',
  'server.vocabularyPermanent': 'ஒரு நிலையான சொல்லகராதியைத் திருத்த முடியாது',
  'server.vocabularyFormatUnknown':
    'கோப்பு வடிவத்தைக் கண்டறிய முடியவில்லை. பட்டியலிலிருந்து வடிவத்தைத் தேர்ந்தெடுக்கவும்.',
  'server.vocabularyFormatInvalid': '"{format}" என்பது சொல்லகராதி கோப்பு வடிவம் அல்ல.',
  'server.vocabularyUrlScheme': 'http மற்றும் https URL-களை மட்டுமே இறக்குமதி செய்ய முடியும்.',
  'server.vocabularyUrlPrivate':
    'URL ஒரு தனிப்பட்ட அல்லது உள்ளூர் பிணைய முகவரியைக் குறிப்பிடுகிறது, அதை சர்வர் பெறாது.',
  'server.vocabularyUrlTooLarge': 'URL-இல் உள்ள கோப்பு {size}-ஐ விடப் பெரியது.',
  'server.vocabularyUrlTimeout': 'URL-இல் உள்ள சர்வர் சரியான நேரத்தில் பதிலளிக்கவில்லை.',
  'server.vocabularyUrlStatus': 'URL-இல் உள்ள சர்வர் {status} நிலையைத் திருப்பியது.',
  'server.vocabularyChangeStale':
    '"{name}" ஒப்பீட்டிற்குப் பிறகு மாறியுள்ளது. கோப்பை மீண்டும் ஒப்பிடவும்.',
  'server.vocabularyMemberMissing': '"{name}" இந்த சொல்லகராதியின் உறுப்பினர் அல்ல.',
  'server.nameEmpty': 'பெயர் காலியாக இருக்கக்கூடாது.',
  'server.emailTaken': 'மின்னஞ்சல் {email} ஏற்கனவே பயன்படுத்தப்பட்டுள்ளது.',
  'server.userRoleRequired': 'பயனர்களுக்கு ஒரு பங்கு இருக்க வேண்டும்.',
  'server.invalidEmail':
    'உள்ளீடு சரியான மின்னஞ்சல் முகவரி அல்ல. அடிப்படை வடிவத்தைப் பயன்படுத்தவும் local-part@hostname',
  'server.siteTitleRequired': 'ஒரு தளம் தலைப்பைக் கொண்டிருக்க வேண்டும்.',
  'server.siteThemeRequired': 'ஒரு தளம் ஒரு கருப்பொருளைக் கொண்டிருக்க வேண்டும்.',
  'server.siteItemPoolRequired': 'ஒரு தளம் உருப்படித் தொகுப்புத் தரவைக் கொண்டிருக்க வேண்டும்.',
  'server.homepageNotInSite': 'ஒரு முகப்புப் பக்கம் அதன் தாய் தளத்தைச் சேர்ந்ததாக இருக்க வேண்டும்.',
  'server.slugEmpty': 'ஸ்லக் காலியாக இருக்கக்கூடாது.',
  'server.slugInvalid':
    'ஒரு ஸ்லக் எழுத்துக்கள், எண்கள், அடிக்கோடுகள் மற்றும் ஹைபன்களை மட்டுமே கொண்டிருக்க முடியும்.',
  'server.slugTaken': 'ஸ்லக் "{slug}" ஏற்கனவே பயன்படுத்தப்பட்டுள்ளது.',
  'server.pageSiteRequired': 'ஒரு பக்கம் ஒரு தளத்தைச் சேர்ந்ததாக இருக்க வேண்டும்.',
  'server.pageTitleRequired': 'ஒரு பக்கம் தலைப்பைக் கொண்டிருக்க வேண்டும்.',
  'server.blockLayoutRequired': 'அனைத்துத் தொகுதிகளும் ஒரு அமைப்பைக் கொண்டிருக்க வேண்டும்.',
  'server.blockDataScalar': 'பிளாக் தரவு ஒரு ஸ்கேலார் மதிப்பாக இருக்கக்கூடாது.',
  'server.siteSettingsNotObject': 'எதிர்பார்க்கப்பட்ட பொருள் அல்லது null, {kind} கிடைத்தது.',
  'server.cssTooLarge':
    'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: தனிப்பயன் CSS அதிகபட்சம் {max} பைட்டுகளாக இருக்கலாம்; இது {bytes} ஆகும்.',
  'server.cssImport': 'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: @import அனுமதிக்கப்படாது.',
  'server.cssDocument': 'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: @document அனுமதிக்கப்படாது.',
  'server.cssForeignUrl':
    'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: url({url}) அனுமதிக்கப்படாது; ஒரு URL என்பது /files/ அல்லது /assets/ இன் கீழ் ஒரு பாதையாக (path) இருக்க வேண்டும்.',
  'server.cssExpression':
    'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: expression() அனுமதிக்கப்படாது.',
  'server.cssBehavior': 'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: behavior அனுமதிக்கப்படாது.',
  'server.cssMozBinding':
    'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: -moz-binding அனுமதிக்கப்படாது.',
  'server.cssStyleBreakout': 'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: </style அனுமதிக்கப்படாது.',
  'server.cssBadUrl': 'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: இந்த url( சரியான CSS அல்ல.',
  'server.cssUnclosedString':
    'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: இந்தச் சரம் மூடப்படவில்லை.',
  'server.cssStrayClose': 'தனிப்பயன் CSS: வரி {line}, நிரல் {column}: {bracket} எதையும் மூடாது.',
  'server.navNotArray': 'தவறான வழிசெலுத்தல்: வழிசெலுத்தல் ஒரு வரிசையாக (array) இருக்க வேண்டும்',
  'server.navLinkMissingType': 'தவறான வழிசெலுத்தல்: இணைப்பு வகை விடுபட்டுள்ளது',
  'server.navLinkMissingData': 'தவறான வழிசெலுத்தல்: இணைப்பு தரவு விடுபட்டுள்ளது',
  'server.navInvalidLinkData': 'தவறான வழிசெலுத்தல்: தவறான இணைப்பு தரவு',
  'server.navPageLinksNotUnique': 'தவறான வழிசெலுத்தல்: பக்க இணைப்புகள் தனித்துவமாக இருக்க வேண்டும்',
  'server.navLinksNotArray': 'தவறான வழிசெலுத்தல்: இணைப்புகள் ஒரு வரிசையாக (array) இருக்க வேண்டும்',
  'server.navPageLinkMissingPage': 'தவறான வழிசெலுத்தல்: பக்க இணைப்பு விடுபட்டுள்ளது',
  'server.navUrlMissingLabel': 'தவறான வழிசெலுத்தல்: URL இணைப்பு விடுபட்டுள்ளது',
  'server.navUrlMissingUrl': 'தவறான வழிசெலுத்தல்: URL இணைப்பில் URL இல்லை',
  'server.navUrlInvalidCharacter': 'தவறான வழிசெலுத்தல்: URL இணைப்பு தவறான எழுத்தைக் கொண்டுள்ளது',
  'server.navUrlInvalidScheme':
    'தவறான வழிசெலுத்தல்: URL இணைப்பு தவறான திட்டத்தைக் (scheme) கொண்டுள்ளது',
  'server.settingsDenied': 'தற்போதைய பயனருக்கு உலகளாவிய அமைப்புகளை அணுக அனுமதி மறுக்கப்பட்டது.',
  'server.settingsNotObject': 'ஒரு பொருளை (object) எதிர்பார்த்தேன்.',
  'server.settingUnavailable': '{key} என்ற அமைப்பானது கிடைக்கவில்லை.',
  'server.settingNotText': '{key} என்பது ஒரு சரமாக (string) அல்லது null ஆக இருக்க வேண்டும்.',
  'server.settingNotFlag': '{key} என்பது true, false அல்லது null ஆக இருக்க வேண்டும்.',
  'server.settingNotNames':
    '{key} என்பது சரங்களின் பட்டியலாக (list of strings) அல்லது null ஆக இருக்க வேண்டும்.',
  'server.publicationTermsRemoved':
    'வெளியிடப்பட்ட ஒரு கலைச்சொல்லை நீக்கவோ அல்லது மறுபெயரிடவோ முடியாது: {terms}. அதற்குப் பதிலாக அதை நீக்கவும் (deprecate).',
  'server.publicationNamespaceFixed':
    'ஒரு வெளியிடப்பட்ட சொல்லகராதி அதன் பெயரிடைவெளியைத் (namespace) தக்கவைத்துக் கொள்கிறது, எனவே அதன் கலைச்சொற்கள் அவற்றின் URIs-ஐத் தக்கவைத்துக் கொள்கின்றன.',
  'server.publicationNotDeletable':
    'வெளியிடப்பட்ட ஒரு கலைச்சொல் பட்டியல் அல்லது கலைச்சொல் தொகுப்பை நீக்க முடியாது. அதற்குப் பதிலாக அதன் கலைச்சொற்களை நீக்கவும் (deprecate).',
  'server.publicationDeprecationFixed':
    '"{term}" என்ற கலைச்சொல்லின் நீக்கம் (deprecation) வெளியிடப்பட்டுவிட்டது, அதைத் திரும்பப் பெற முடியாது.',
  'server.publicationItemsList':
    'உருப்படிகளின் பட்டியல் வெளியிடப்பட முடியாது. ஒவ்வொரு உருப்படியும் ஏற்கனவே அதன் சொந்த முகவரியைக் கொண்டுள்ளது.',
  'server.publicationForeignNamespace':
    '{namespace} என்ற பெயர்வெளி (namespace) இந்த ஆவணக் காப்பகத்தின் பெயர்வெளி அல்ல. இந்த ஆவணக் காப்பகத்தின் /vocab/ முகவரிக்குக் கீழ் ஒரு கலைச்சொல் தொகுப்பை வெளியிடவும், அல்லது w3id.org அல்லது purl.org இல் வெளியிடவும்.',
  'server.publicationSlugInvalid':
    'ஒரு முகவரிப் பெயர் சிறிய எழுத்துக்கள், எண்கள் மற்றும் ஒற்றை ஹைபன்களைப் பயன்படுத்துகிறது, அதிகபட்சம் 64 எழுத்துக்கள்.',
  'server.publicationSlugTaken': '/vocab/{slug}/ என்ற முகவரி ஏற்கனவே எடுக்கப்பட்டுவிட்டது.',
  'server.publicationNamespaceNotPersistent':
    'இந்த ஆவணக் காப்பகத்தின் பெயர்வெளி அல்லாத ஒரு பெயர்வெளி w3id.org அல்லது purl.org இல் இருக்க வேண்டும் மற்றும் / அல்லது # உடன் முடிய வேண்டும்.',
  'server.publicationAddressFixed':
    'ஒரு வெளியிடப்பட்ட கலைச்சொல் பட்டியல் அல்லது கலைச்சொல் தொகுப்பு அது முதலில் வெளியிடப்பட்ட முகவரியைத் தக்கவைத்துக் கொள்கிறது.',
  'server.publicationTermNotPublished':
    '"{term}" இன்னும் வெளியிடப்படவில்லை, எனவே அதை நீக்க முடியாது. அதற்குப் பதிலாக அதை நீக்கவும்.',
  'server.publicationUnknownTerm':
    '"{term}" இந்த பட்டியலின் அல்லது கலைச்சொல் தொகுப்பின் உறுப்பு அல்ல.',
  'server.publicationMatchInvalid':
    'ஒரு பொருத்தம் என்பது முழுமையான http அல்லது https URI ஆக இருக்க வேண்டும்.',
  'server.publicationMatchTermLists':
    'கலைச்சொல் பட்டியலின் ஒரு உறுப்பு மட்டுமே ஒரு அதிகாரத்துடன் இணைக்க முடியும்.',
  'server.publicationTermIriInvalid': '{uri} என்ற உறுப்பு URI ஒரு சரியான IRI அல்ல.',
  'server.importAdminsOnly':
    'ஒரு ஆவணக் காப்பகத்தை இறக்குமதி செய்ய உலகளாவிய அல்லது தள நிர்வாகி மட்டுமே முடியும்.',
  'server.importNotFound': '{id} என்ற அடையாளத்தைக் கொண்ட இறக்குமதி எதுவும் இல்லை.',
  'server.importRequestUnreadable': 'கோரிக்கையைப் படிக்க முடியாது: {reason}',
  'server.importPlanRefused': 'இறக்குமதியின் அமைப்புகளைச் சேமிக்க முடியாது: {reason}',
  'server.importStale':
    'நீங்கள் இதைத் திறந்த பிறகு, வேறு யாரோ இந்த இறக்குமதியைச் சேமித்துள்ளனர். அவர்களின் மாற்றங்களைக் காண இதை மீண்டும் ஏற்றவும்.',
  'server.importPlanFrozen':
    'இந்த இறக்குமதி ஏற்கனவே தொடங்கிவிட்டது, எனவே அதன் அமைப்புகளை மாற்ற முடியாது. அதை மாற்ற, ஒரு நகலை உருவாக்கவும்.',
  'server.importV1Plan':
    'இந்த இறக்குமதி பழைய இறக்குமதியாளரால் செய்யப்பட்டது, எனவே இதை இங்கே திறக்கவோ அல்லது நகலெடுக்கவோ முடியாது.',
  'server.importTypeExists':
    'Omeka இல் "{label}" என்ற பெயரில் ஒரு உருப்படி வகை ஏற்கனவே உள்ளது. புதிய வகைக்கு மற்றொரு பெயரைத் தேர்ந்தெடுக்கவும்.',
  'server.importDeleteNotDraft':
    'இன்னும் சேர்க்கப்படாத ஒரு இறக்குமதியை மட்டுமே நீக்க முடியும். இயங்கிய ஒரு இறக்குமதி முந்தைய இறக்குமதிகளில் இருக்கும்.',
  'server.importChanged':
    'கடைசிச் சரிபார்ப்பிற்குப் பிறகு கோப்புகள் அல்லது இறக்குமதியின் அமைப்புகள் மாறின. அதை மீண்டும் இயக்குவதற்கு முன் இறக்குமதியை மீண்டும் சரிபார்க்கவும்.',
  'server.importBlocked':
    'சரிபார்ப்புப் பட்டியலில் உள்ள சிக்கல்கள் சரிசெய்யப்படும் வரை இந்த இறக்குமதியை இயக்க முடியாது.',
  'server.importNothingToRun': 'இந்த இறக்குமதியில் எழுத எதுவும் இல்லை.',
  'server.importConfirmDelete':
    'இந்த இறக்குமதி {count} உருப்படிகளை, ஊடகங்கள் மற்றும் உருப்படித் தொகுப்புகளை நீக்குகிறது, மேலும் இதை ரத்து செய்ய முடியாது. இதை இயக்க அந்த எண்ணிக்கையை உறுதிப்படுத்தவும்.',
  'server.importBusy':
    'மற்றொரு இறக்குமதி இயங்குகிறது அல்லது ரத்து செய்யப்படுகிறது. அது முடிந்ததும் மீண்டும் முயற்சிக்கவும்.',
  'server.importCannotRun':
    'இன்னும் சேர்க்கப்படாத, முடிவதற்கு முன் நின்ற அல்லது தோல்விகளுடன் முடிந்த இறக்குமதிகளை மட்டுமே இயக்க முடியும்.',
  'server.importCannotUndo':
    'முடிந்த அல்லது முடிவதற்கு முன் நின்ற இறக்குமதிகளை மட்டுமே ரத்து செய்ய முடியும்.',
  'server.importNotRunning': 'இந்த இறக்குமதி இயங்கவில்லை.',
  'server.importNoRoot': '"{root}" என்று பெயரிடப்பட்ட காப்பகக் கோப்புறை எதுவும் இல்லை.',
  'server.importOutsideRoot': '"{path}" என்ற கோப்புறை காப்பகக் கோப்புறைக்கு வெளியே உள்ளது.',
  'server.importFolderMissing': 'காப்பகத்தில் "{path}" என்ற கோப்புறை இல்லை.',
  'server.importNotAFolder': '"{path}" என்பது ஒரு கோப்பு, கோப்புறை அல்ல.',
  'server.importScanBusy':
    'இந்த இறக்குமதி இயங்குகிறது அல்லது ரத்து செய்யப்படுகிறது. அது முடிந்ததும் மீண்டும் கோப்புகளைப் பார்க்கவும்.',
  'server.importNoLeftFile':
    'இந்த இறக்குமதியில் இன்னும் விடுபட்ட கோப்புகளின் பட்டியல் இல்லை. ஒரு ஓட்டம் முடிந்ததும் இது எழுதப்படும்.',
  'server.importJobNotStarted': 'இறக்குமதி தொடங்க முடியவில்லை: {reason}',
  'server.importListColumnUnknown':
    'பட்டியலில் "{column}" என்ற நெடுவரிசை இருக்க முடியாது. ஒரு பண்பு, உருப்படித் தொகுப்பு, உருப்படி வகை அல்லது தெரிவுத்திறனைத் தேர்ந்தெடுக்கவும்.',
  'server.importListOneFile': 'ஒரு பட்டியல் கோப்பை படிவத்தில் அனுப்பவும்.',
  'server.importListType':
    '"{name}" என்பது .csv, .tsv, .xlsx, .xls அல்லது .ods கோப்பு அல்ல. பட்டியலை இவற்றுள் ஒன்றாகச் சேமித்து, பின்னர் மீண்டும் அனுப்பவும்.',
  'server.importListTooLarge': 'பட்டியல் 50 MB-க்கும் அதிகமாக உள்ளது.',
  'server.importListUnread': 'பட்டியலை வாசிக்க முடியவில்லை: {reason}',
  'server.importListNoIdentifier':
    'பட்டியலில் அடையாளங்காட்டி (Identifier) நெடுவரிசை இல்லை, எனவே அதன் வரிசைகள் அவற்றின் உருப்படிகளைக் கண்டறிய முடியாது. நீங்கள் பதிவிறக்கிய பட்டியலின் அடையாளங்காட்டி நெடுவரிசையை வைத்திருக்கவும்.',
  'server.importPairsColumns':
    'பட்டியலில் Row நெடுவரிசையும் Files நெடுவரிசையும் தேவை. நீங்கள் பதிவிறக்கம் செய்த பட்டியலின் இரண்டு நெடுவரிசைகளையும் அப்படியே வைத்திருக்கவும்.',
  'server.importFileUnreadable': '"{path}" கோப்பை வாசிக்க முடியவில்லை: {reason}',
  'server.importFileRefused': '"{path}" கோப்பு சேர்க்கப்படவில்லை: {reason}',
  'server.importFileNotStored': '"{path}" கோப்பை சேமிக்க முடியவில்லை: {reason}',
  'server.importSourceRefused': '"{source}" சேர்க்கப்படவில்லை: {reason}',
  'server.importMediaNoSource':
    'இந்த ஊடகத்தில் சேர்க்க எந்தக் கோப்பும் முகவரியும் குறிப்பிடப்படவில்லை.',
  'server.importMediaNoItem':
    'இந்த ஊடகத்தின் உருப்படி சேர்க்கப்படவில்லை, எனவே ஊடகமும் சேர்க்கப்படவில்லை.',
  'server.importResourceGone':
    'இந்த பதிவு மாற்றும் உருப்படி, ஊடகம் அல்லது உருப்படித் தொகுப்பு இனி Omeka-வில் இல்லை.',
  'server.importRecordNotSaved': 'இந்த பதிவைச் சேமிக்க முடியவில்லை: {reason}',
  'server.importLinksNotSaved': 'இந்த பதிவின் இணைப்புகளைச் சேமிக்க முடியவில்லை: {reason}',
  'server.importRecordNotDeleted': 'இது நீக்கப்படவில்லை: {reason}',
  'server.importDeleteNotUndone':
    'உருப்படிகள், ஊடகங்கள் அல்லது உருப்படித் தொகுப்புகளை நீக்கிய ஒரு இறக்குமதியை ரத்து செய்ய முடியாது.',
  'server.importSitesNotJoined':
    'இந்த உருப்படி சேர்க்கப்பட்டது, ஆனால் அதன் தளங்களில் இணைக்கப்படவில்லை: {reason}',
  'server.importUndoEdited': 'இறக்குமதி இதை எழுதிய பிறகு இது திருத்தப்பட்டது.',
  'server.importUndoLinked':
    'இந்த இறக்குமதிக்கு வெளியே உள்ள ஒரு உருப்படி, ஊடகம் அல்லது உருப்படித் தொகுப்பு இதனுடன் இணைக்கப்பட்டுள்ளது.',
  'server.importUndoMediaAdded':
    'இந்த இறக்குமதிக்கு வெளியே இந்த உருப்படியில் ஊடகங்கள் சேர்க்கப்பட்டன.',
  'server.importUndoMembersAdded':
    'இந்த இறக்குமதிக்கு வெளியே இருந்த உருப்படிகள் இந்த உருப்படித் தொகுப்பில் சேர்க்கப்பட்டன.',
  'server.importUndoLaterImport': 'இந்த இறக்குமதிக்குப் பிறகு இறக்குமதி {import} இதை மாற்றியது.',
  'server.importUndoInUse':
    'இந்த இறக்குமதிக்கு வெளியே உள்ள உருப்படிகள், ஊடகங்கள், உருப்படித் தொகுப்புகள் அல்லது உருப்படி வகைகள் இதைப் பயன்படுத்துகின்றன.',
  'server.importUndoItemKept':
    'இந்த ஊடகத்தின் உருப்படி அப்படியே வைக்கப்படுகிறது, எனவே ஊடகமும் அப்படியே வைக்கப்படுகிறது.',
  'server.importUndoMemberKept':
    'இந்த உருப்படித் தொகுப்பில் உள்ள ஒரு உருப்படி வைக்கப்படுகிறது, எனவே உருப்படித் தொகுப்பும் வைக்கப்படுகிறது.',
  'server.importUndoStopped': 'ரத்து செய்தல் முடிவதற்கு முன்பே நின்றுவிட்டது: {reason}',
};
