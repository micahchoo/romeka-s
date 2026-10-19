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
};
