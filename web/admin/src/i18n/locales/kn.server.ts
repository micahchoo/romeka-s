/**
 * The Kannada server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Kannada. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const knServer: ServerCatalogue = {
  'server.methodNotAllowed': 'ವಿಧಾನವು ಅನುಮತಿಸಲ್ಪಟ್ಟಿಲ್ಲ',
  'server.notFound': 'ಕಂಡುಬಂದಿಲ್ಲ',
  'server.badRequest': 'ಕೆಟ್ಟ ವಿನಂತಿ',
  'server.requestTimeout': 'ವಿನಂತಿಯ ಸಮಯ ಮೀರಿದೆ',
  'server.bodyTooLarge': 'ವಿನಂತಿಯ ದೇಹವು ತುಂಬಾ ದೊಡ್ಡದಾಗಿದೆ',
  'server.internalError': 'ಆಂತರಿಕ ಸರ್ವರ್ ದೋಷ',
  'server.tooManyRequests': 'ತುಂಬಾ ವಿನಂತಿಗಳು',
  'server.unknownResource': 'API ಯು "{resource}" ಸಂಪನ್ಮೂಲವನ್ನು ಬೆಂಬಲಿಸುವುದಿಲ್ಲ.',
  'server.adapterNotImplemented': '{resource} ಅಡಾಪ್ಟರ್ {operation} ಕಾರ್ಯಾಚರಣೆಯನ್ನು ಅಳವಡಿಸುವುದಿಲ್ಲ.',
  'server.endpointNotImplemented': '{endpoint} ಅಂತ್ಯಬಿಂದುವನ್ನು ಇನ್ನೂ ಅಳವಡಿಸಲಾಗಿಲ್ಲ.',
  'server.permissionDenied':
    'ಪ್ರಸ್ತುತ ಬಳಕೆದಾರರಿಗೆ {resource} ಸಂಪನ್ಮೂಲವನ್ನು {operation} ಮಾಡಲು ಅನುಮತಿ ಇಲ್ಲ.',
  'server.entityNotFound': '{class} ಮಾನದಂಡ {criteria} ನೊಂದಿಗೆ ಕಂಡುಬಂದಿಲ್ಲ',
  'server.jsonSyntax': 'JSON: ಸಿಂಟ್ಯಾಕ್ಸ್ ದೋಷ',
  'server.jsonNotContainer': 'JSON: ವಿಷಯವು ಒಂದು ವಸ್ತು ಅಥವಾ ಅರೇ ಆಗಿರಬೇಕು.',
  'server.invalidContentType':
    'ಅಮಾನ್ಯವಾದ Content-Type ಹೆಡರ್. "application/json" ನಿರೀಕ್ಷಿಸಲಾಗುತ್ತಿದೆ, ಆದರೆ "{received}" ಬಂದಿದೆ.',
  'server.bodyNotObject': 'ವಿನಂತಿಯ ದೇಹವು ಒಂದು JSON ವಸ್ತುವಾಗಿರಬೇಕು.',
  'server.siteItemsOneSelector': 'ಹೆಸರು "query", "items" ಅಥವಾ "item_sets" ಗಳಲ್ಲಿ ಒಂದಾಗಿರಬೇಕು.',
  'server.invalidCredentials': 'ಇಮೇಲ್ ಅಥವಾ ಪಾಸ್‌ವರ್ಡ್ ಅಮಾನ್ಯವಾಗಿದೆ',
  'server.inactiveAccount': 'ಬಳಕೆದಾರರು ಸಕ್ರಿಯವಾಗಿಲ್ಲ.',
  'server.unknownKey': 'ಅಮಾನ್ಯವಾದ ಪಾಸ್‌ವರ್ಡ್ ರಚನೆಯ ಕೀ.',
  'server.authenticationRequired': 'ದೃಢೀಕರಣ ಅಗತ್ಯವಿದೆ.',
  'server.invalidPassword': 'ಪಾಸ್‌ವರ್ಡ್ ಅಮಾನ್ಯವಾಗಿದೆ',
  'server.crossOriginRefused': 'ಸೆಷನ್-ದೃಢೀಕೃತ ಬರವಣಿಗೆಯು ಕಾನ್ಫಿಗರ್ ಮಾಡಿದ ಬೇಸ್ URL ನಿಂದಲೇ ಬರಬೇಕು.',
  'server.propertyNotRecognized': '"{term}" ಗುಣಲಕ್ಷಣವನ್ನು ಗುರುತಿಸಲಾಗಿಲ್ಲ.',
  'server.resourceNotFound': '{id} ಐಡಿ ಯೊಂದಿಗೆ ಸಂಪನ್ಮೂಲ ಕಂಡುಬಂದಿಲ್ಲ.',
  'server.templateRequiresValue':
    '"{template}" ಸಂಪನ್ಮೂಲ ಟೆಂಪ್ಲೇಟ್‌ಗೆ "{property}" ಮೌಲ್ಯದ ಅಗತ್ಯವಿದೆ',
  'server.itemSetsNotArray': 'ಐಟಂ ಸೆಟ್‌ಗಳು ಅರೇ ಆಗಿರಬೇಕು',
  'server.mediaNotArray': 'ಮೀಡಿಯಾ ಅರೇ ಆಗಿರಬೇಕು',
  'server.mediaNotThisItem': 'ಮೀಡಿಯಾ ಈ ಐಟಂಗೆ ಸೇರಿರಬೇಕು.',
  'server.primaryMediaNotThisItem': 'ಪ್ರಾಥಮಿಕ ಮೀಡಿಯಾ ಈ ಐಟಂಗೆ ಸೇರಿರಬೇಕು.',
  'server.mediaIngesterRequired': 'ಮೀಡಿಯಾ ಇಂಜೆಸ್ಟರ್ ಅನ್ನು ಹೊಂದಿರಬೇಕು.',
  'server.mediaIngesterInvalid': 'ಮೀಡಿಯಾ ಮಾನ್ಯವಾದ ಇಂಜೆಸ್ಟರ್ ಅನ್ನು ಹೊಂದಿರಬೇಕು.',
  'server.mediaItemRequired': 'ಮೀಡಿಯಾ ಒಂದು ಐಟಂಗೆ ಸೇರಿರಬೇಕು.',
  'server.noFileUploaded': 'ಯಾವುದೇ ಫೈಲ್ ಅಪ್‌ಲೋಡ್ ಆಗಿಲ್ಲ',
  'server.assetMediaType':
    '"{file}" ಅನ್ನು ದೃಢೀಕರಿಸುವಲ್ಲಿ ದೋಷ. "{media}" ಮಾಧ್ಯಮ ಪ್ರಕಾರದೊಂದಿಗೆ ಫೈಲ್‌ಗಳನ್ನು ಸಂಗ್ರಹಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ.',
  'server.assetExtension':
    '"{file}" ಅನ್ನು ದೃಢೀಕರಿಸುವಲ್ಲಿ ದೋಷ. ಪರಿಹರಿಸಿದ ವಿಸ್ತರಣೆ "{extension}" ನೊಂದಿಗೆ ಫೈಲ್‌ಗಳನ್ನು ಸಂಗ್ರಹಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ.',
  'server.assetNameRequired': 'ಆಸ್ತಿಯು ಒಂದು ಹೆಸರನ್ನು ಹೊಂದಿರಬೇಕು.',
  'server.classesNotArray': 'ವರ್ಗಗಳು ಒಂದು ಅರೇ ಆಗಿರಬೇಕು',
  'server.propertiesNotArray': 'ಗುಣಲಕ್ಷಣಗಳು ಒಂದು ಅರೇ ಆಗಿರಬೇಕು',
  'server.namespaceUriEmpty': 'ನೇಮ್‌ಸ್ಪೇಸ್ ಯುಆರ್‌ಐ ಖಾಲಿ ಇರಬಾರದು.',
  'server.namespaceUriTaken': 'ನೇಮ್‌ಸ್ಪೇಸ್ ಯುಆರ್‌ಐ "{uri}" ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.prefixEmpty': 'ಪ್ರಿಫಿಕ್ಸ್ ಖಾಲಿ ಇರಬಾರದು.',
  'server.prefixInvalid':
    'ಪ್ರಿಫಿಕ್ಸ್ ಕೇವಲ ಅಕ್ಷರಗಳು, ಸಂಖ್ಯೆಗಳು, ಹೈಫನ್‌ಗಳು ಮತ್ತು ಅಂಡರ್‌ಸ್ಕೋರ್‌ಗಳನ್ನು ಮಾತ್ರ ಹೊಂದಿರಬಹುದು.',
  'server.prefixTaken': 'ಪ್ರಿವಿಕ್ಸ "{prefix}" ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.prefixReserved': 'ಪ್ರಿವಿಕ್ಸ "{prefix}" ಕಾಯ್ದಿರಿಸಲಾಗಿದೆ.',
  'server.labelEmpty': 'ಲೇಬಲ್ ಖಾಲಿ ಇರಬಾರದು.',
  'server.labelTaken': 'ಲೇಬಲ್ ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.localNameEmpty': 'ಸ್ಥಳೀಯ ಹೆಸರು ಖಾಲಿ ಇರಬಾರದು.',
  'server.localNameTaken': 'ಸ್ಥಳೀಯ ಹೆಸರು "{name}" ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.vocabularyRequired': 'ಶಬ್ದಕೋಶವನ್ನು ನಿಗದಿಪಡಿಸಬೇಕು.',
  'server.duplicateTemplateProperty':
    'ಐಡಿ {id} ನೊಂದಿಗೆ ನಕಲಿ ಗುಣಲಕ್ಷಣವನ್ನು ಸೇರಿಸಲು ಪ್ರಯತ್ನಿಸಲಾಗುತ್ತಿದೆ',
  'server.nameEmpty': 'ಹೆಸರು ಖಾಲಿ ಇರಬಾರದು.',
  'server.emailTaken': 'ಇಮೇಲ್ {email} ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.userRoleRequired': 'ಬಳಕೆದಾರರಿಗೆ ಪಾತ್ರವಿರಬೇಕು.',
  'server.invalidEmail':
    'ಇನ್‌ಪುಟ್ ಮಾನ್ಯವಾದ ಇಮೇಲ್ ವಿಳಾಸವಲ್ಲ. ಮೂಲಭೂತ ಸ್ವರೂಪ local-part@hostname ಬಳಸಿ',
  'server.siteTitleRequired': 'ಸೈಟ್‌ಗೆ ಶೀರ್ಷಿಕೆ ಇರಬೇಕು.',
  'server.siteThemeRequired': 'ಸೈಟ್‌ಗೆ ಥೀಮ್ ಇರಬೇಕು.',
  'server.siteItemPoolRequired': 'ಸೈಟ್‌ಗೆ ಐಟಂ ಪೂಲ್ ಡೇಟಾ ಇರಬೇಕು.',
  'server.homepageNotInSite': 'ಮುಖಪುಟವು ಅದರ ಪೋಷಕ ಸೈಟ್‌ಗೆ ಸೇರಿರಬೇಕು.',
  'server.slugEmpty': 'ಸ್ಲಗ್ ಖಾಲಿ ಇರಬಾರದು.',
  'server.slugInvalid':
    'ಸ್ಲಗ್ ಕೇವಲ ಅಕ್ಷರಗಳು, ಸಂಖ್ಯೆಗಳು, ಅಂಡರ್‌ಸ್ಕೋರ್‌ಗಳು ಮತ್ತು ಹೈಫನ್‌ಗಳನ್ನು ಮಾತ್ರ ಹೊಂದಿರಬಹುದು.',
  'server.slugTaken': 'ಸ್ಲಗ್ "{slug}" ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.pageSiteRequired': 'ಪುಟವು ಸೈಟ್‌ಗೆ ಸೇರಿರಬೇಕು.',
  'server.pageTitleRequired': 'ಪುಟಕ್ಕೆ ಶೀರ್ಷಿಕೆ ಇರಬೇಕು.',
  'server.blockLayoutRequired': 'ಎಲ್ಲಾ ಬ್ಲಾಕ್‌ಗಳಿಗೆ ಲೇಔಟ್ ಇರಬೇಕು.',
  'server.blockDataScalar': 'ಬ್ಲಾಕ್ ಡೇಟಾ ಸ್ಕೇಲಾರ್ ಮೌಲ್ಯವಾಗಿರಬಾರದು.',
  'server.siteSettingsNotObject': 'ನಿರೀಕ್ಷಿತ ಆಬ್ಜೆಕ್ಟ್ ಅಥವಾ ನಲ್, {kind} ದೊರೆತಿದೆ.',
  'server.customCssRefusal': 'ಕಸ್ಟಮ್ CSS: {refusal}',
  'server.navNotArray': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: ನ್ಯಾವಿಗೇಷನ್ ಅರೇ ಆಗಿರಬೇಕು',
  'server.navLinkMissingType': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: ಲಿಂಕ್ ಪ್ರಕಾರ ಕಾಣೆಯಾಗಿದೆ',
  'server.navLinkMissingData': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: ಲಿಂಕ್ ಡೇಟಾ ಕಾಣೆಯಾಗಿದೆ',
  'server.navInvalidLinkData': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಷನ್: ಅಮಾನ್ಯ ಲಿಂಕ್ ಡೇಟಾ',
  'server.navPageLinksNotUnique': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: ಪುಟ ಲಿಂಕ್‌ಗಳು ವಿಶಿಷ್ಟವಾಗಿರಬೇಕು',
  'server.navLinksNotArray': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: ಲಿಂಕ್‌ಗಳು ಅರೇ ಆಗಿರಬೇಕು',
  'server.navPageLinkMissingPage': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: ಪುಟ ಲಿಂಕ್ ಪುಟ ಐಡಿ ಕಾಣೆಯಾಗಿದೆ',
  'server.navUrlMissingLabel': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: URL ಲಿಂಕ್ ಲೇಬಲ್ ಕಾಣೆಯಾಗಿದೆ',
  'server.navUrlMissingUrl': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: URL ಲಿಂಕ್ URL ಕಾಣೆಯಾಗಿದೆ',
  'server.navUrlInvalidCharacter': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: URL ಲಿಂಕ್ ಅಮಾನ್ಯ ಅಕ್ಷರವನ್ನು ಹೊಂದಿದೆ',
  'server.navUrlInvalidScheme': 'ಅಮಾನ್ಯ ನ್ಯಾವಿಗೇಷನ್: URL ಲಿಂಕ್ ಅಮಾನ್ಯ ಸ್ಕೀಮ್ ಹೊಂದಿದೆ',
  'server.settingsDenied':
    'ಪ್ರಸ್ತುತ ಬಳಕೆದಾರರಿಗೆ ಜಾಗತಿಕ ಸೆಟ್ಟಿಂಗ್‌ಗಳನ್ನು ಪ್ರವೇಶಿಸಲು ಅನುಮತಿ ನಿರಾಕರಿಸಲಾಗಿದೆ.',
  'server.settingsNotObject': 'ನಿರೀಕ್ಷಿತ ಒಂದು ಆಬ್ಜೆಕ್ಟ್.',
  'server.settingUnavailable': 'ಸೆಟ್ಟಿಂಗ್ {key} ಲಭ್ಯವಿಲ್ಲ.',
  'server.settingNotText': 'ಸೆಟ್ಟಿಂಗ್ {key} ಸ್ಟ್ರಿಂಗ್ ಅಥವಾ ನಲ್ ಆಗಿರಬೇಕು.',
};
