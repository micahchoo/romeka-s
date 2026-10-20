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
  'server.jobHeldByImport':
    'ಈ ಕೆಲಸವು ಚಾಲನೆಯಲ್ಲಿರುವ ಅಥವಾ ರದ್ದುಗೊಳಿಸುತ್ತಿರುವ ಆರ್ಕೈವ್ ಇಂಪೋರ್ಟ್ಗೆ ಸೇರಿದೆ. ಇಂಪೋರ್ಟ್ ಮುಗಿದ ನಂತರ ಅದನ್ನು ಅಳಿಸಿ.',
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
  'server.vocabularyNoFile': 'ಫೈಲ್ ಆಮದು ತಂತ್ರಕ್ಕಾಗಿ ಯಾವುದೇ ಫೈಲ್ ನಿರ್ದಿಷ್ಟಪಡಿಸಿಲ್ಲ.',
  'server.vocabularyNoUrl': 'URL ಆಮದು ತಂತ್ರಕ್ಕಾಗಿ ಯಾವುದೇ URL ನಿರ್ದಿಷ್ಟಪಡಿಸಿಲ್ಲ.',
  'server.vocabularyNotParsed': 'ಶಬ್ದಕೋಶದ ಫೈಲ್ ಅನ್ನು ವಿಶ್ಲೇಷಿಸಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ.',
  'server.vocabularyNotLoaded': 'URL ನಿಂದ ಶಬ್ದಕೋಶವನ್ನು ಲೋಡ್ ಮಾಡಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ.',
  'server.vocabularyNoMembers':
    'ಯಾವುದೇ ವರ್ಗಗಳು ಅಥವಾ ಗುಣಲಕ್ಷಣಗಳು ಕಂಡುಬಂದಿಲ್ಲ. ನೀವು ಸರಿಯಾದ ನೇಮ್‌ಸ್ಪೇಸ್ URI ಬಳಸಿದ್ದೀರಾ?',
  'server.vocabularyPermanent': 'ಶಾಶ್ವತ ಶಬ್ದಕೋಶವನ್ನು ಸಂಪಾದಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ',
  'server.vocabularyFormatUnknown':
    'ಫೈಲ್ ಫಾರ್ಮ್ಯಾಟ್ ಅನ್ನು ಪತ್ತೆಹಚ್ಚಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ. ಪಟ್ಟಿಯಿಂದ ಫಾರ್ಮ್ಯಾಟ್ ಅನ್ನು ಆರಿಸಿ.',
  'server.vocabularyFormatInvalid': '"{format}" ಶಬ್ದಕೋಶದ ಫೈಲ್ ಫಾರ್ಮ್ಯಾಟ್ ಅಲ್ಲ.',
  'server.vocabularyUrlScheme': 'ಕೇವಲ http ಮತ್ತು https URL ಗಳನ್ನು ಆಮದು ಮಾಡಿಕೊಳ್ಳಬಹುದು.',
  'server.vocabularyUrlPrivate':
    'URL ಒಂದು ಖಾಸಗಿ ಅಥವಾ ಸ್ಥಳೀಯ ನೆಟ್‌ವರ್ಕ್ ವಿಳಾಸವನ್ನು ಸೂಚಿಸುತ್ತದೆ, ಇದನ್ನು ಸರ್ವರ್ ಪಡೆಯುವುದಿಲ್ಲ.',
  'server.vocabularyUrlTooLarge': 'URL ನಲ್ಲಿರುವ ಫೈಲ್ {size} ಗಿಂತ ದೊಡ್ಡದಾಗಿದೆ.',
  'server.vocabularyUrlTimeout': 'URL ನಲ್ಲಿರುವ ಸರ್ವರ್ ಸಮಯಕ್ಕೆ ಸರಿಯಾಗಿ ಪ್ರತಿಕ್ರಿಯಿಸಲಿಲ್ಲ.',
  'server.vocabularyUrlStatus': 'URL ನಲ್ಲಿರುವ ಸರ್ವರ್ {status} ಸ್ಥಿತಿಯೊಂದಿಗೆ ಪ್ರತಿಕ್ರಿಯಿಸಿದೆ.',
  'server.vocabularyChangeStale':
    '"{name}" ಅನ್ನು ಹೋಲಿಕೆ ಮಾಡಿದ ನಂತರ ಬದಲಾಗಿದೆ. ಫೈಲ್ ಅನ್ನು ಮತ್ತೆ ಹೋಲಿಸಿ.',
  'server.vocabularyMemberMissing': '"{name}" ಈ ಶಬ್ದಕೋಶದ ಸದಸ್ಯನಲ್ಲ.',
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
  'server.cssTooLarge':
    'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: ಕಸ್ಟಮ್ CSS ಗರಿಷ್ಠ {max} ಬೈಟ್‌ಗಳಷ್ಟಿರಬಹುದು; ಇದು {bytes}.',
  'server.cssImport': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: @import ಅನುಮತಿಸಲಾಗಿಲ್ಲ.',
  'server.cssDocument': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: @document ಅನುಮತಿಸಲಾಗಿಲ್ಲ.',
  'server.cssForeignUrl':
    'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: url({url}) ಅನುಮತಿಸಲಾಗಿಲ್ಲ; ಒಂದು URL /files/ ಅಥವಾ /assets/ ಅಡಿಯಲ್ಲಿ ಒಂದು ಹಾದಿಯಾಗಿರಬೇಕು.',
  'server.cssExpression': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: expression() ಅನುಮತಿಸಲಾಗಿಲ್ಲ.',
  'server.cssBehavior': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: behavior ಅನುಮತಿಸಲಾಗಿಲ್ಲ.',
  'server.cssMozBinding': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: -moz-binding ಅನುಮತಿಸಲಾಗಿಲ್ಲ.',
  'server.cssStyleBreakout': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: </style ಅನುಮತಿಸಲಾಗಿಲ್ಲ.',
  'server.cssBadUrl': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: ಈ url( ಮಾನ್ಯವಾದ CSS ಅಲ್ಲ.',
  'server.cssUnclosedString':
    'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: ಈ ಸ್ಟ್ರಿಂಗ್ ಮುಚ್ಚಲ್ಪಟ್ಟಿಲ್ಲ.',
  'server.cssStrayClose': 'ಕಸ್ಟಮ್ CSS: ಸಾಲು {line}, ಕಾಲಮ್ {column}: {bracket} ಏನೂ ಮುಚ್ಚುವುದಿಲ್ಲ.',
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
  'server.settingNotFlag': 'ಸೆಟ್ಟಿಂಗ್ {key} ಸತ್ಯ, ಸುಳ್ಳು ಅಥವಾ ನಲ್ ಆಗಿರಬೇಕು.',
  'server.settingNotNames': 'ಸೆಟ್ಟಿಂಗ್ {key} ಸ್ಟ್ರಿಂಗ್‌ಗಳ ಪಟ್ಟಿಯಾಗಿರಬೇಕು ಅಥವಾ ನಲ್ ಆಗಿರಬೇಕು.',
  'server.publicationTermsRemoved':
    'ಪ್ರಕಟಿತ ಪದವನ್ನು ತೆಗೆದುಹಾಕಲು ಅಥವಾ ಮರುನಾಮಕರಣ ಮಾಡಲು ಸಾಧ್ಯವಿಲ್ಲ: {terms}. ಅದನ್ನು ಬದಲಿಗೆ ಡಿಪ್ರಿಕೇಟ್ ಮಾಡಿ.',
  'server.publicationNamespaceFixed':
    'ಪ್ರಕಟಿತ ಶಬ್ದಕೋಶವು ಅದರ ನೇಮ್‌ಸ್ಪೇಸ್ ಅನ್ನು ಉಳಿಸಿಕೊಳ್ಳುತ್ತದೆ, ಆದ್ದರಿಂದ ಅದರ ಪದಗಳು ತಮ್ಮ URI ಗಳನ್ನು ಉಳಿಸಿಕೊಳ್ಳುತ್ತವೆ.',
  'server.publicationNotDeletable':
    'ಪ್ರಕಟಿತ ಪದಗಳ ಪಟ್ಟಿ ಅಥವಾ ಶಬ್ದಕೋಶವನ್ನು ಅಳಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ. ಅದರ ಪದಗಳನ್ನು ಬದಲಿಗೆ ಡಿಪ್ರಿಕೇಟ್ ಮಾಡಿ.',
  'server.publicationDeprecationFixed':
    '"{term}" ನ ಡಿಪ್ರಿಕೇಶನ್ ಪ್ರಕಟವಾಗಿದೆ ಮತ್ತು ಅದನ್ನು ಹಿಂಪಡೆಯಲು ಸಾಧ್ಯವಿಲ್ಲ.',
  'server.publicationItemsList':
    'ಐಟಂಗಳ ಪಟ್ಟಿಯನ್ನು ಪ್ರಕಟಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ. ಪ್ರತಿ ಐಟಂ ಈಗಾಗಲೇ ತನ್ನದೇ ಆದ ವಿಳಾಸವನ್ನು ಹೊಂದಿದೆ.',
  'server.publicationForeignNamespace':
    'ನೇಮ್‌ಸ್ಪೇಸ್ {namespace} ಈ ಆರ್ಕೈವ್‌ನದ್ದಲ್ಲ. ಈ ಆರ್ಕೈವ್‌ನ /vocab/ ವಿಳಾಸದ ಅಡಿಯಲ್ಲಿ ಬರುವ ಶಬ್ದಕೋಶವನ್ನು ಪ್ರಕಟಿಸಿ, ಅಥವಾ w3id.org ಅಥವಾ purl.org ನಲ್ಲಿ ಪ್ರಕಟಿಸಿ.',
  'server.publicationSlugInvalid':
    'ವಿಳಾಸದ ಹೆಸರು ಸಣ್ಣ ಅಕ್ಷರಗಳು, ಅಂಕಿಗಳು ಮತ್ತು ಒಂದೇ ಹೈಫನ್ ಅನ್ನು ಬಳಸುತ್ತದೆ, ಗರಿಷ್ಠ 64 ಅಕ್ಷರಗಳು.',
  'server.publicationSlugTaken': 'ವಿಳಾಸ /vocab/{slug}/ ಈಗಾಗಲೇ ಬಳಕೆಯಲ್ಲಿದೆ.',
  'server.publicationNamespaceNotPersistent':
    'ಈ ಆರ್ಕೈವ್‌ನದ್ದಲ್ಲದ ನೇಮ್‌ಸ್ಪೇಸ್ w3id.org ಅಥವಾ purl.org ನಲ್ಲಿರಬೇಕು ಮತ್ತು ಕೊನೆಯಲ್ಲಿ / ಅಥವಾ # ನೊಂದಿಗೆ ಕೊನೆಗೊಳ್ಳಬೇಕು.',
  'server.publicationAddressFixed':
    'ಪ್ರಕಟಿತ ಪದಗಳ ಪಟ್ಟಿ ಅಥವಾ ಶಬ್ದಕೋಶವು ಅದು ಮೊದಲು ಪ್ರಕಟವಾದ ವಿಳಾಸವನ್ನು ಉಳಿಸಿಕೊಳ್ಳುತ್ತದೆ.',
  'server.publicationTermNotPublished':
    '"{term}" ಇನ್ನೂ ಪ್ರಕಟವಾಗಿಲ್ಲ, ಆದ್ದರಿಂದ ಅದನ್ನು ಡಿಪ್ರಿಕೇಟ್ ಮಾಡಲು ಸಾಧ್ಯವಿಲ್ಲ. ಅದನ್ನು ತೆಗೆದುಹಾಕಿ.',
  'server.publicationUnknownTerm': '"{term}" ಈ ಪಟ್ಟಿಯ ಅಥವಾ ಶಬ್ದಕೋಶದ ಪದವಲ್ಲ.',
  'server.publicationMatchInvalid': 'ಹೊಂದಾಣಿಕೆಯು ಸಂಪೂರ್ಣ http ಅಥವಾ https URI ಆಗಿರಬೇಕು.',
  'server.publicationMatchTermLists': 'ಕೇವಲ ಪದಗಳ ಪಟ್ಟಿಯ ಪದಗಳು ಮಾತ್ರ ಅಧಿಕಾರಕ್ಕೆ ಲಿಂಕ್ ಆಗುತ್ತವೆ.',
  'server.publicationTermIriInvalid': 'ಪದದ URI {uri} ಮಾನ್ಯವಾದ IRI ಅಲ್ಲ.',
  'server.importAdminsOnly':
    'ಕೇವಲ ಜಾಗತಿಕ ಅಥವಾ ಸೈಟ್ ನಿರ್ವಾಹಕರು ಮಾತ್ರ ಆರ್ಕೈವ್ ಅನ್ನು ಇಂಪೋರ್ಟ್ ಮಾಡಬಹುದು.',
  'server.importNotFound': 'ಯಾವುದೇ ಇಂಪೋರ್ಟ್ {id} ಐಡಿಯನ್ನು ಹೊಂದಿಲ್ಲ.',
  'server.importRequestUnreadable': 'ವಿನಂತಿಯನ್ನು ಓದಲು ಸಾಧ್ಯವಿಲ್ಲ: {reason}',
  'server.importPlanRefused': 'ಇಂಪೋರ್ಟ್‌ನ ಸೆಟ್ಟಿಂಗ್‌ಗಳನ್ನು ಉಳಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ: {reason}',
  'server.importStale':
    'ನೀವು ಇದನ್ನು ತೆರೆದ ನಂತರ ಬೇರೊಬ್ಬರು ಈ ಇಂಪೋರ್ಟ್ ಅನ್ನು ಉಳಿಸಿದ್ದಾರೆ. ಅವರ ಬದಲಾವಣೆಗಳನ್ನು ನೋಡಲು ಇದನ್ನು ಮರುಲೋಡ್ ಮಾಡಿ.',
  'server.importPlanFrozen':
    'ಈ ಇಂಪೋರ್ಟ್ ಈಗಾಗಲೇ ಪ್ರಾರಂಭವಾಗಿದೆ, ಆದ್ದರಿಂದ ಅದರ ಸೆಟ್ಟಿಂಗ್‌ಗಳು ಬದಲಾಗಲು ಸಾಧ್ಯವಿಲ್ಲ. ಇದನ್ನು ಬದಲಾಯಿಸಲು ಒಂದು ಪ್ರತಿಯನ್ನು ಮಾಡಿ.',
  'server.importV1Plan':
    'ಈ ಇಂಪೋರ್ಟ್ ಹಳೆಯ ಇಂಪೋರ್ಟರ್‌ನಿಂದ ಮಾಡಲ್ಪಟ್ಟಿದೆ, ಆದ್ದರಿಂದ ಇದನ್ನು ಇಲ್ಲಿ ತೆರೆಯಲು ಅಥವಾ ನಕಲಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ.',
  'server.importTypeExists':
    'Omeka ನಲ್ಲಿ "{label}" ಹೆಸರಿನ ಒಂದು ಐಟಂ ಪ್ರಕಾರ ಈಗಾಗಲೇ ಇದೆ. ಹೊಸ ಪ್ರಕಾರಕ್ಕಾಗಿ ಮತ್ತೊಂದು ಹೆಸರನ್ನು ಆರಿಸಿ.',
  'server.importDeleteNotDraft':
    'ಇನ್ನೂ ಸೇರಿಸದ ಇಂಪೋರ್ಟ್ ಅನ್ನು ಮಾತ್ರ ಅಳಿಸಬಹುದು. ರನ್ ಆಗಿರುವ ಇಂಪೋರ್ಟ್ ಹಿಂದಿನ ಇಂಪೋರ್ಟ್‌ಗಳಲ್ಲಿ ಇರುತ್ತದೆ.',
  'server.importChanged':
    'ಕೊನೆಯ ಪರಿಶೀಲನೆಯ ನಂತರ ಫೈಲ್‌ಗಳು ಅಥವಾ ಇಂಪೋರ್ಟ್‌ನ ಸೆಟ್ಟಿಂಗ್‌ಗಳು ಬದಲಾಗಿವೆ. ನೀವು ಇದನ್ನು ರನ್ ಮಾಡುವ ಮೊದಲು ಇಂಪೋರ್ಟ್ ಅನ್ನು ಮತ್ತೊಮ್ಮೆ ಪರಿಶೀಲಿಸಿ.',
  'server.importBlocked':
    'ಪರಿಶೀಲನೆ ಪಟ್ಟಿ ಮಾಡುವ ಸಮಸ್ಯೆಗಳನ್ನು ಸರಿಪಡಿಸುವವರೆಗೆ ಈ ಇಂಪೋರ್ಟ್ ರನ್ ಆಗಲು ಸಾಧ್ಯವಿಲ್ಲ.',
  'server.importNothingToRun': 'ಈ ಇಂಪೋರ್ಟ್ ಬರೆಯಲು ಏನೂ ಇಲ್ಲ.',
  'server.importConfirmDelete':
    'ಈ ಇಂಪೋರ್ಟ್ {count} ಐಟಂಗಳನ್ನು, ಮೀಡಿಯಾ ಮತ್ತು ಐಟಂ ಸೆಟ್‌ಗಳನ್ನು ಅಳಿಸುತ್ತದೆ ಮತ್ತು ಇದನ್ನು ರದ್ದುಗೊಳಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ. ಇದನ್ನು ರನ್ ಮಾಡಲು ಆ ಸಂಖ್ಯೆಯೊಂದಿಗೆ ದೃಢೀಕರಿಸಿ.',
  'server.importBusy':
    'ಮತ್ತೊಂದು ಇಂಪೋರ್ಟ್ ರನ್ ಆಗುತ್ತಿದೆ ಅಥವಾ ರದ್ದುಗೊಳಿಸಲ್ಪಡುತ್ತಿದೆ. ಅದು ಮುಗಿದಾಗ ಮತ್ತೆ ಪ್ರಯತ್ನಿಸಿ.',
  'server.importCannotRun':
    'ಇನ್ನೂ ಸೇರಿಸದ, ಮೊದಲೇ ನಿಲ್ಲಿಸಿದ ಅಥವಾ ವೈಫಲ್ಯಗಳೊಂದಿಗೆ ಪೂರ್ಣಗೊಂಡ ಇಂಪೋರ್ಟ್ ಮಾತ್ರ ರನ್ ಆಗಬಹುದು.',
  'server.importCannotUndo': 'ಪೂರ್ಣಗೊಂಡ ಅಥವಾ ಮೊದಲೇ ನಿಲ್ಲಿಸಿದ ಇಂಪೋರ್ಟ್ ಅನ್ನು ಮಾತ್ರ ರದ್ದುಗೊಳಿಸಬಹುದು.',
  'server.importNotRunning': 'ಈ ಇಂಪೋರ್ಟ್ ರನ್ ಆಗುತ್ತಿಲ್ಲ.',
  'server.importNoRoot': '"{root}" ಹೆಸರಿನ ಯಾವುದೇ ಆರ್ಕೈವ್ ಫೋಲ್ಡರ್ ಇಲ್ಲ.',
  'server.importOutsideRoot': '"{path}" ಫೋಲ್ಡರ್ ಆರ್ಕೈವ್ ಫೋಲ್ಡರ್‌ನ ಹೊರಗಿದೆ.',
  'server.importFolderMissing': 'ಆರ್ಕೈವ್‌ನಲ್ಲಿ "{path}" ಫೋಲ್ಡರ್ ಇಲ್ಲ.',
  'server.importNotAFolder': '"{path}" ಒಂದು ಫೈಲ್ ಆಗಿದೆ, ಫೋಲ್ಡರ್ ಅಲ್ಲ.',
  'server.importScanBusy':
    'ಈ ಇಂಪೋರ್ಟ್ ರನ್ ಆಗುತ್ತಿದೆ ಅಥವಾ ರದ್ದುಗೊಳಿಸಲ್ಪಡುತ್ತಿದೆ. ಅದು ಮುಗಿದಾಗ ಫೈಲ್‌ಗಳನ್ನು ಮತ್ತೊಮ್ಮೆ ನೋಡಿ.',
  'server.importNoLeftFile':
    'ಈ ಇಂಪೋರ್ಟ್ ಇನ್ನೂ ಬಿಟ್ಟುಬಿಟ್ಟ ಫೈಲ್‌ಗಳ ಪಟ್ಟಿಯನ್ನು ಹೊಂದಿಲ್ಲ. ರನ್ ಮುಗಿದಾಗ ಇದನ್ನು ಬರೆಯಲಾಗುತ್ತದೆ.',
  'server.importJobNotStarted': 'ಇಂಪೋರ್ಟ್ ಪ್ರಾರಂಭಿಸಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ: {reason}',
  'server.importListColumnUnknown':
    'ಪಟ್ಟಿಯಲ್ಲಿ "{column}" ಕಾಲಮ್ ಇರಲು ಸಾಧ್ಯವಿಲ್ಲ. ಒಂದು ಪ್ರಾಪರ್ಟಿ, ಐಟಂ ಸೆಟ್‌ಗಳು, ಐಟಂ ಪ್ರಕಾರ ಅಥವಾ ವಿಸಿಬಿಲಿಟಿಯನ್ನು ಆರಿಸಿ.',
  'server.importListOneFile': 'ಫಾರ್ಮ್‌ನಲ್ಲಿ ಒಂದು ಪಟ್ಟಿ ಫೈಲ್ ಅನ್ನು ಕಳುಹಿಸಿ.',
  'server.importListType':
    '"{name}" ಎಂಬುದು .csv, .tsv, .xlsx, .xls ಅಥವಾ .ods ಫೈಲ್ ಅಲ್ಲ. ಪಟ್ಟಿಯನ್ನು ಇವುಗಳಲ್ಲಿ ಒಂದಾಗಿ ಉಳಿಸಿ, ನಂತರ ಅದನ್ನು ಮತ್ತೆ ಕಳುಹಿಸಿ.',
  'server.importListTooLarge': 'ಪಟ್ಟಿಯು 50 MB ಗಿಂತ ದೊಡ್ಡದಾಗಿದೆ.',
  'server.importListUnread': 'ಪಟ್ಟಿಯನ್ನು ಓದಲು ಸಾಧ್ಯವಾಗುತ್ತಿಲ್ಲ: {reason}',
  'server.importListNoIdentifier':
    'ಪಟ್ಟಿಯಲ್ಲಿ ಐಡೆಂಟಿಫೈಯರ್ ಕಾಲಮ್ ಇಲ್ಲ, ಆದ್ದರಿಂದ ಅದರ ಸಾಲುಗಳು ತಮ್ಮ ಐಟಂಗಳನ್ನು ಹುಡುಕಲು ಸಾಧ್ಯವಿಲ್ಲ. ನೀವು ಡೌನ್‌ಲೋಡ್ ಮಾಡಿದ ಪಟ್ಟಿಯ ಐಡೆಂಟಿಫೈಯರ್ ಕಾಲಮ್ ಅನ್ನು ಇರಿಸಿ.',
  'server.importPairsColumns':
    'ಪಟ್ಟಿಗೆ Row ಕಾಲಮ್ ಮತ್ತು Files ಕಾಲಮ್ ಬೇಕು. ನೀವು ಡೌನ್‌ಲೋಡ್ ಮಾಡಿದ ಪಟ್ಟಿಯ ಎರಡೂ ಕಾಲಮ್‌ಗಳನ್ನು ಹಾಗೆಯೇ ಇರಿಸಿ.',
  'server.importFileUnreadable': '"{path}" ಫೈಲ್ ಅನ್ನು ಓದಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ: {reason}',
  'server.importFileRefused': '"{path}" ಫೈಲ್ ಅನ್ನು ಸೇರಿಸಲಾಗಿಲ್ಲ: {reason}',
  'server.importFileNotStored': '"{path}" ಫೈಲ್ ಅನ್ನು ಸಂಗ್ರಹಿಸಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ: {reason}',
  'server.importSourceRefused': '"{source}" ಅನ್ನು ಸೇರಿಸಲಾಗಿಲ್ಲ: {reason}',
  'server.importMediaNoSource': 'ಈ ಮೀಡಿಯಾಕ್ಕೆ ಸೇರಿಸಲು ಯಾವುದೇ ಫೈಲ್ ಅಥವಾ ವಿಳಾಸವನ್ನು ಹೆಸರಿಸಿಲ್ಲ.',
  'server.importMediaNoItem':
    'ಈ ಮೀಡಿಯಾದ ಐಟಂ ಅನ್ನು ಸೇರಿಸಲಾಗಿಲ್ಲ, ಆದ್ದರಿಂದ ಮೀಡಿಯಾವನ್ನು ಸಹ ಸೇರಿಸಲಾಗಿಲ್ಲ.',
  'server.importResourceGone': 'ಈ ದಾಖಲೆ ಬದಲಾಯಿಸುವ ಐಟಂ, ಮೀಡಿಯಾ ಅಥವಾ ಐಟಂ ಸೆಟ್ ಈಗ Omeka ನಲ್ಲಿ ಇಲ್ಲ.',
  'server.importRecordNotSaved': 'ಈ ದಾಖಲೆಯನ್ನು ಉಳಿಸಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ: {reason}',
  'server.importLinksNotSaved': 'ಈ ದಾಖಲೆಯ ಲಿಂಕ್‌ಗಳನ್ನು ಉಳಿಸಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ: {reason}',
  'server.importRecordNotDeleted': 'ಇದನ್ನು ಅಳಿಸಲಾಗಿಲ್ಲ: {reason}',
  'server.importDeleteNotUndone':
    'ಐಟಂಗಳು, ಮೀಡಿಯಾ ಅಥವಾ ಐಟಂ ಸೆಟ್‌ಗಳನ್ನು ಅಳಿಸಿದ ಇಂಪೋರ್ಟ್ ಅನ್ನು ರದ್ದುಗೊಳಿಸಲು ಸಾಧ್ಯವಿಲ್ಲ.',
  'server.importSitesNotJoined':
    'ಈ ಐಟಂ ಅನ್ನು ಸೇರಿಸಲಾಗಿದೆ, ಆದರೆ ಅದನ್ನು ಅದರ ಸೈಟ್‌ಗಳಿಗೆ ಸೇರಿಸಲಾಗಿಲ್ಲ: {reason}',
  'server.importUndoEdited': 'ಆಮದು ಇದನ್ನು ಬರೆದ ನಂತರ ಇದನ್ನು ಸಂಪಾದಿಸಲಾಗಿದೆ.',
  'server.importUndoLinked': 'ಈ ಆಮದಿನ ಹೊರಗಿನ ಒಂದು ಐಟಂ, ಮೀಡಿಯಾ ಅಥವಾ ಐಟಂ ಸೆಟ್ ಇದಕ್ಕೆ ಲಿಂಕ್ ಆಗಿದೆ.',
  'server.importUndoMediaAdded': 'ಈ ಆಮದಿನ ಹೊರಗಿನಿಂದ ಈ ಐಟಂಗೆ ಮೀಡಿಯಾವನ್ನು ಸೇರಿಸಲಾಗಿದೆ.',
  'server.importUndoMembersAdded': 'ಈ ಆಮದಿನ ಹೊರಗಿನ ಐಟಂಗಳನ್ನು ಈ ಐಟಂ ಸೆಟ್‌ಗೆ ಸೇರಿಸಲಾಗಿದೆ.',
  'server.importUndoLaterImport': 'ಈ ಆಮದಿನ ನಂತರ ಆಮದು {import} ಇದನ್ನು ಬದಲಾಯಿಸಿದೆ.',
  'server.importUndoInUse':
    'ಈ ಆಮದಿನ ಹೊರಗಿನ ಐಟಂಗಳು, ಮೀಡಿಯಾ, ಐಟಂ ಸೆಟ್‌ಗಳು ಅಥವಾ ಐಟಂ ಪ್ರಕಾರಗಳು ಇದನ್ನು ಬಳಸುತ್ತವೆ.',
  'server.importUndoItemKept':
    'ಈ ಮೀಡಿಯಾದ ಐಟಂ ಅನ್ನು ಉಳಿಸಿಕೊಳ್ಳಲಾಗುತ್ತದೆ, ಆದ್ದರಿಂದ ಮೀಡಿಯಾವನ್ನು ಸಹ ಉಳಿಸಿಕೊಳ್ಳಲಾಗುತ್ತದೆ.',
  'server.importUndoMemberKept':
    'ಈ ಐಟಂ ಸೆಟ್‌ನಲ್ಲಿರುವ ಐಟಂ ಅನ್ನು ಉಳಿಸಿಕೊಳ್ಳಲಾಗುತ್ತದೆ, ಆದ್ದರಿಂದ ಐಟಂ ಸೆಟ್ ಅನ್ನು ಸಹ ಉಳಿಸಿಕೊಳ್ಳಲಾಗುತ್ತದೆ.',
  'server.importUndoStopped': 'ರದ್ದುಗೊಳಿಸುವಿಕೆ ಮುಗಿಯುವ ಮೊದಲೇ ನಿಂತಿತು: {reason}',
};
