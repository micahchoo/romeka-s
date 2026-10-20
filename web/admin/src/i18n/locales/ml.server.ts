/**
 * The Malayalam server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Malayalam. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const mlServer: ServerCatalogue = {
  'server.methodNotAllowed': 'രീതി അനുവദനീയമല്ല',
  'server.notFound': 'കണ്ടെത്താനായില്ല',
  'server.badRequest': 'തെറ്റായ അഭ്യർത്ഥന',
  'server.requestTimeout': 'അഭ്യർത്ഥന സമയപരിധി',
  'server.bodyTooLarge': 'അഭ്യർത്ഥനയുടെ വലിപ്പം കൂടുതലാണ്',
  'server.internalError': 'ആന്തരിക സെർവർ പിശക്',
  'server.tooManyRequests': 'വളരെയധികം അഭ്യർത്ഥനകൾ',
  'server.unknownResource': 'API "{resource}" വിഭവത്തെ പിന്തുണയ്ക്കുന്നില്ല.',
  'server.adapterNotImplemented':
    '{resource} അഡാപ്റ്റർ {operation} പ്രവർത്തനം നടപ്പിലാക്കുന്നില്ല.',
  'server.endpointNotImplemented': '{endpoint} എൻഡ്‌പോയിന്റ് ഇതുവരെ നടപ്പിലാക്കിയിട്ടില്ല.',
  'server.permissionDenied':
    'നിലവിലെ ഉപയോക്താവിന് {operation} ചെയ്യാൻ {resource} വിഭവത്തിന് അനുമതിയില്ല.',
  'server.entityNotFound': '{criteria} മാനദണ്ഡങ്ങളുള്ള {class} എന്റിറ്റി കണ്ടെത്താനായില്ല',
  'server.jsonSyntax': 'JSON: സിന്റാക്സ് പിശക്',
  'server.jsonNotContainer': 'JSON: ഉള്ളടക്കം ഒരു ഒബ്‌ജക്റ്റോ അറേയോ ആയിരിക്കണം.',
  'server.invalidContentType':
    'അസാധുവായ Content-Type ഹെഡർ. "application/json" പ്രതീക്ഷിക്കുന്നു, "{received}" ലഭിച്ചു.',
  'server.bodyNotObject': 'അഭ്യർത്ഥനയുടെ ബോഡി ഒരു JSON ഒബ്‌ജക്റ്റായിരിക്കണം.',
  'server.siteItemsOneSelector': '"query", "items", "item_sets" അല്ലെങ്കിൽ രണ്ടും ആയിരിക്കണം പേര്',
  'server.invalidCredentials': 'ഇമെയിൽ അല്ലെങ്കിൽ പാസ്‌വേഡ് അസാധുവാണ്',
  'server.inactiveAccount': 'ഉപയോക്താവ് സജീവമല്ല.',
  'server.unknownKey': 'അസാധുവായ പാസ്‌വേഡ് നിർമ്മാണ കീ.',
  'server.authenticationRequired': 'ഓതന്റിക്കേഷൻ ആവശ്യമാണ്.',
  'server.invalidPassword': 'പാസ്‌വേഡ് അസാധുവാണ്',
  'server.crossOriginRefused':
    'ഒരു സെഷൻ-ഓതന്റിക്കേറ്റഡ് റൈറ്റ് കോൺഫിഗർ ചെയ്ത ബേസ് URL-ൽ നിന്ന് വരണം.',
  'server.propertyNotRecognized': '"{term}" എന്ന പ്രോപ്പർട്ടി തിരിച്ചറിയുന്നില്ല.',
  'server.resourceNotFound': '{id} ഐഡി ഉപയോഗിച്ച് റിസോഴ്സ് കണ്ടെത്താനായില്ല.',
  'server.templateRequiresValue':
    '"{template}" റിസോഴ്സ് ടെംപ്ലേറ്റിന് ഒരു "{property}" മൂല്യം ആവശ്യമാണ്',
  'server.itemSetsNotArray': 'ഐറ്റം സെറ്റുകൾ ഒരു അറേ ആയിരിക്കണം',
  'server.mediaNotArray': 'മീഡിയ ഒരു അറേ ആയിരിക്കണം',
  'server.mediaNotThisItem': 'മീഡിയ ഈ ഐറ്റത്തിന്റേതായിരിക്കണം.',
  'server.primaryMediaNotThisItem': 'പ്രൈമറി മീഡിയ ഈ ഐറ്റത്തിന്റേതായിരിക്കണം.',
  'server.mediaIngesterRequired': 'മീഡിയ ഒരു ഇൻഗെസ്റ്റർ നിശ്ചയിക്കണം.',
  'server.mediaIngesterInvalid': 'മീഡിയ ഒരു സാധുവായ ഇൻഗെസ്റ്റർ നിശ്ചയിക്കണം.',
  'server.mediaItemRequired': 'മീഡിയ ഒരു ഐറ്റത്തിന്റേതായിരിക്കണം.',
  'server.jobHeldByImport':
    'ഈ ജോലി നടക്കുന്നതോ റദ്ദാക്കപ്പെടുന്നതോ ആയ ഒരു ആർക്കൈവ് ഇംപോർട്ടിന്റേതാണ്. ഇംപോർട്ട് അവസാനിച്ച ശേഷം ഇത് ഇല്ലാതാക്കുക.',
  'server.pageSavedSinceOpened': 'നിങ്ങൾ ഈ പേജ് തുറന്നതിന് ശേഷം മറ്റൊരാൾ അത് സേവ് ചെയ്തു.',
  'server.noFileUploaded': 'ഫയൽ അപ്‌ലോഡ് ചെയ്തിട്ടില്ല',
  'server.assetMediaType':
    '"{file}" സാധൂകരിക്കുന്നതിൽ പിശക്. "{media}" മീഡിയ ടൈപ്പിലുള്ള ഫയലുകൾ സംഭരിക്കാൻ കഴിയില്ല.',
  'server.assetExtension':
    '"{file}" സാധൂകരിക്കുന്നതിൽ പിശക്. "{extension}" എക്സ്റ്റൻഷനുള്ള ഫയലുകൾ സംഭരിക്കാൻ കഴിയില്ല.',
  'server.assetNameRequired': 'ഒരു അസറ്റിന് പേര് ഉണ്ടായിരിക്കണം.',
  'server.classesNotArray': 'ക്ലാസുകൾ ഒരു അറേ ആയിരിക്കണം',
  'server.propertiesNotArray': 'പ്രോപ്പർട്ടികൾ ഒരു അറേ ആയിരിക്കണം',
  'server.namespaceUriEmpty': 'നെയിംസ്പേസ് URI ശൂന്യമാകാൻ പാടില്ല.',
  'server.namespaceUriTaken': 'നെയിംസ്പേസ് URI "{uri}" നേരത്തെ ഉപയോഗിച്ചു.',
  'server.prefixEmpty': 'പ്രിഫിക്സ് ശൂന്യമാകാൻ പാടില്ല.',
  'server.prefixInvalid':
    'പ്രിഫിക്സിൽ അക്ഷരങ്ങൾ, അക്കങ്ങൾ, ഹൈഫനുകൾ, അണ്ടർസ്കോറുകൾ എന്നിവ മാത്രമേ ഉണ്ടാകാൻ പാടുള്ളൂ.',
  'server.prefixTaken': '"{prefix}" എന്ന പ്രിഫിക്സ് ഉപയോഗിച്ചു.',
  'server.prefixReserved': '"{prefix}" എന്നഫിക്സ് റിസർവ് ആണ്.',
  'server.labelEmpty': 'ലേബൽ ശൂന്യമാകാൻ പാടില്ല.',
  'server.labelTaken': 'ലേബൽ ഉപയോഗിച്ചു.',
  'server.localNameEmpty': 'ലോക്കൽ നെയിം ശൂന്യമാകാൻ പാടില്ല.',
  'server.localNameTaken': '"{name}" എന്ന ലോക്കൽ നെയിം ഉപയോഗിച്ചു.',
  'server.vocabularyRequired': 'ഒരു വോക്കബുലറി നിശ്ചയിക്കണം.',
  'server.duplicateTemplateProperty':
    '{id} ഐഡി ഉള്ള ഡ്യൂപ്ലിക്കേറ്റ് പ്രോപ്പർട്ടി ചേർക്കാൻ ശ്രമിക്കുന്നു',
  'server.vocabularyNoFile': 'ഫയൽ ഇംപോർട്ട് സ്ട്രാറ്റജിക്കായി ഫയൽ നൽകിയിട്ടില്ല.',
  'server.vocabularyNoUrl': 'URL ഇംപോർട്ട് സ്ട്രാറ്റജിക്കായി URL നൽകിയിട്ടില്ല.',
  'server.vocabularyNotParsed': 'വൊക്കാബുലറി ഫയൽ വിശകലനം ചെയ്യാൻ കഴിഞ്ഞില്ല.',
  'server.vocabularyNotLoaded': 'URL-ൽ നിന്ന് വൊക്കാബുലറി ലോഡ് ചെയ്യാൻ കഴിഞ്ഞില്ല.',
  'server.vocabularyNoMembers':
    'ക്ലാസുകളോ പ്രോപ്പർട്ടികളോ കണ്ടെത്തിയില്ല. നിങ്ങൾ ശരിയായ നെയിംസ്‌പേസ് URI ഉപയോഗിച്ചോ?',
  'server.vocabularyPermanent': 'ഒരു സ്ഥിരമായ വൊക്കാബുലറി എഡിറ്റ് ചെയ്യാൻ കഴിയില്ല',
  'server.vocabularyFormatUnknown':
    'ഫയൽ ഫോർമാറ്റ് കണ്ടെത്താനായില്ല. പട്ടികയിൽ നിന്ന് ഫോർമാറ്റ് തിരഞ്ഞെടുക്കുക.',
  'server.vocabularyFormatInvalid': '"{format}" വൊക്കാബുലറി ഫയൽ ഫോർമാറ്റ് അല്ല.',
  'server.vocabularyUrlScheme': 'http, https URL-കൾ മാത്രമേ ഇംപോർട്ട് ചെയ്യാൻ കഴിയൂ.',
  'server.vocabularyUrlPrivate':
    'URL ഒരു സ്വകാര്യ അല്ലെങ്കിൽ ലോക്കൽ നെറ്റ്‌വർക്ക് വിലാസത്തെ സൂചിപ്പിക്കുന്നു, സെർവർ അത് ശേഖരിക്കില്ല.',
  'server.vocabularyUrlTooLarge': 'URL-ലെ ഫയൽ {size}-നേക്കാൾ വലുതാണ്.',
  'server.vocabularyUrlTimeout': 'URL-ലെ സെർവർ സമയത്തിനുള്ളിൽ മറുപടി നൽകിയില്ല.',
  'server.vocabularyUrlStatus': 'URL-ലെ സെർവർ {status} എന്ന സ്റ്റാറ്റസോടെ മറുപടി നൽകി.',
  'server.vocabularyChangeStale':
    '"{name}" താരതമ്യത്തിന് ശേഷം മാറിയിട്ടുണ്ട്. ഫയൽ വീണ്ടും താരതമ്യം ചെയ്യുക.',
  'server.vocabularyMemberMissing': '"{name}" ഈ വൊക്കാബുലറിയിൽ അംഗമല്ല.',
  'server.nameEmpty': 'പേര് ശൂന്യമാകാൻ പാടില്ല.',
  'server.emailTaken': '{email} എന്ന ഇമെയിൽ ഉപയോഗിച്ചു.',
  'server.userRoleRequired': 'ഉപയോക്താക്കൾക്ക് ഒരു റോൾ ഉണ്ടായിരിക്കണം.',
  'server.invalidEmail':
    'ഇൻപുട്ട് ഒരു സാധുവായ ഇമെയിൽ വിലാസമല്ല. അടിസ്ഥാന ഫോർമാറ്റ് local-part@hostname ഉപയോഗിക്കുക.',
  'server.siteTitleRequired': 'ഒരു സൈറ്റിന് ഒരു ടൈറ്റിൽ വേണം.',
  'server.siteThemeRequired': 'ഒരു സൈറ്റിന് ഒരു തീം വേണം.',
  'server.siteItemPoolRequired': 'ഒരു സൈറ്റിന് ഐറ്റം പൂൾ ഡാറ്റ വേണം.',
  'server.homepageNotInSite': 'ഒരു ഹോംപേജ് അതിന്റെ പാരന്റ് സൈറ്റിന്റെ ഭാഗമാകണം.',
  'server.slugEmpty': 'സ്ലഗ് ശൂന്യമാകാൻ പാടില്ല.',
  'server.slugInvalid':
    'ഒരു സ്ലഗ് അക്ഷരങ്ങൾ, അക്കങ്ങൾ, അണ്ടർസ്കോറുകൾ, ഹൈഫനുകൾ എന്നിവ മാത്രമേ ഉൾക്കൊള്ളാവൂ.',
  'server.slugTaken': '"{slug}" എന്ന സ്ലഗ് ഇതിനകം ഉപയോഗിച്ചു കഴിഞ്ഞു.',
  'server.pageSiteRequired': 'ഒരു പേജ് ഒരു സൈറ്റിന്റെ ഭാഗമാകണം.',
  'server.pageTitleRequired': 'ഒരു പേജിന് ഒരു ടൈറ്റിൽ വേണം.',
  'server.blockLayoutRequired': 'എല്ലാ ബ്ലോക്കുകൾക്കും ഒരു ലേഔട്ട് വേണം.',
  'server.blockDataScalar': 'ബ്ലോക്ക് ഡാറ്റ ഒരു സ്കെയിലാർ വാല്യൂ ആകാൻ പാടില്ല.',
  'server.siteSettingsNotObject': 'പ്രതീക്ഷിച്ചത് ഒരു ഒബ്‌ജക്റ്റ് അല്ലെങ്കിൽ നൾ, {kind} ലഭിച്ചു.',
  'server.cssTooLarge':
    'കസ്റ്റം CSS: വരി {line}, കോളം {column}: കസ്റ്റം CSS പരമാവധി {max} ബൈറ്റുകളിൽ; ഇത് {bytes} ആണ്.',
  'server.cssImport': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: @import അനുവദനീയമല്ല.',
  'server.cssDocument': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: @document അനുവദനീയമല്ല.',
  'server.cssForeignUrl':
    'കസ്റ്റം CSS: വരി {line}, കോളം {column}: url({url}) അനുവദനീയമല്ല; ഒരു URL /files/ അല്ലെങ്കിൽ /assets/ എന്നതിന് താഴെയുള്ള ഒരു പാതയായിരിക്കണം.',
  'server.cssExpression': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: expression() അനുവദനീയമല്ല.',
  'server.cssBehavior': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: behavior അനുവദനീയമല്ല.',
  'server.cssMozBinding': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: -moz-binding അനുവദനീയമല്ല.',
  'server.cssStyleBreakout': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: </style അനുവദനീയമല്ല.',
  'server.cssBadUrl': 'കസ്റ്റം CSS: വരി {line}, കോളം {column}: ഈ url( സാധുവായ CSS അല്ല.',
  'server.cssUnclosedString':
    'കസ്റ്റം CSS: വരി {line}, കോളം {column}: ഈ സ്ട്രിംഗ് ക്ലോസ് ചെയ്തിട്ടില്ല.',
  'server.cssStrayClose':
    'കസ്റ്റം CSS: വരി {line}, കോളം {column}: {bracket} ഒന്നും ക്ലോസ് ചെയ്യുന്നില്ല.',
  'server.navNotArray': 'അസാധുവായ നാവിഗേഷൻ: നാവിഗേഷൻ ഒരു അറേ ആയിരിക്കണം',
  'server.navLinkMissingType': 'അസാധുവായ നാവിഗേഷൻ: ലിങ്ക് ടൈപ്പ് ഇല്ല',
  'server.navLinkMissingData': 'അസാധുവായ നാവിഗേഷൻ: ലിങ്ക് ഡാറ്റ ഇല്ല',
  'server.navInvalidLinkData': 'അസാധുവായ നാവിഗേഷൻ: അസാധുവായ ലിങ്ക് ഡാറ്റ',
  'server.navPageLinksNotUnique': 'അസാധുവായ നാവിഗേഷൻ: പേജ് ലിങ്കുകൾ യുണീക് ആയിരിക്കണം',
  'server.navLinksNotArray': 'അസാധുവായ നാവിഗേഷൻ: ലിങ്കുകൾ ഒരു അറേ ആയിരിക്കണം',
  'server.navPageLinkMissingPage': 'അസാധുവായ നാവിഗേഷൻ: പേജ് ലിങ്കിൽ പേജ് ID ഇല്ല',
  'server.navUrlMissingLabel': 'അസാധുവായ നാവിഗേഷൻ: URL ലിങ്കിൽ ലേബൽ ഇല്ല',
  'server.navUrlMissingUrl': 'അസാധുവായ നാവിഗേഷൻ: URL ലിങ്കിൽ URL ഇല്ല',
  'server.navUrlInvalidCharacter': 'അസാധുവായ നാവിഗേഷൻ: URL ലിങ്കിൽ അസാധുവായ ക്യാരക്ടർ ഉണ്ട്',
  'server.navUrlInvalidScheme': 'അസാധുവായ നാവിഗേഷൻ: URL ലിങ്കിൽ അസാധുവായ സ്കീം ഉണ്ട്',
  'server.settingsDenied': 'നിലവിലെ ഉപയോക്താവിന് ഗ്ലോബൽ സെറ്റിംഗ്‌സ് ആക്‌സസ് ചെയ്യാൻ അനുമതിയില്ല.',
  'server.settingsNotObject': 'ഒരു ഒബ്‌ജക്റ്റ് പ്രതീക്ഷിച്ചു.',
  'server.settingUnavailable': '{key} എന്ന ക്രമീകരണം ലഭ്യമല്ല.',
  'server.settingNotText': '{key} ഒരു സ്ട്രിംഗ് അല്ലെങ്കിൽ നൾ ആയിരിക്കണം.',
  'server.settingNotFlag': '{key} ട്രൂ, ഫോൾസ് അല്ലെങ്കിൽ നൾ ആയിരിക്കണം.',
  'server.settingNotNames': '{key} സ്ട്രിംഗുകളുടെ ഒരു ലിസ്റ്റ് അല്ലെങ്കിൽ നൾ ആയിരിക്കണം.',
  'server.publicationTermsRemoved':
    'പ്രസിദ്ധീകരിച്ച ഒരു പദം നീക്കം ചെയ്യാനോ പുനർനാമകരണം ചെയ്യാനോ കഴിയില്ല: {terms}. പകരം അത് ഡിപ്രിക്കേറ്റ് ചെയ്യുക.',
  'server.publicationNamespaceFixed':
    'ഒരു പ്രസിദ്ധീകരിച്ച പദാവലി അതിന്റെ നെയിംസ്‌പേസ് നിലനിർത്തുന്നു, അതിനാൽ അതിന്റെ പദങ്ങൾ അവയുടെ URIs നിലനിർത്തുന്നു.',
  'server.publicationNotDeletable':
    'ഒരു പ്രസിദ്ധീകരിച്ച പദപ്പട്ടികയോ പദാവലിയോ നീക്കം ചെയ്യാൻ കഴിയില്ല. പകരം അതിന്റെ പദങ്ങൾ ഡിപ്രിക്കേറ്റ് ചെയ്യുക.',
  'server.publicationDeprecationFixed':
    '"{term}"-ന്റെ ഡിപ്രിക്കേഷൻ പ്രസിദ്ധീകരിക്കുകയും അത് പിൻവലിക്കാൻ കഴിയില്ല.',
  'server.publicationItemsList':
    'ഇനങ്ങളുടെ ഒരു പട്ടിക പ്രസിദ്ധീകരിക്കാൻ കഴിയില്ല. ഓരോ ഇനത്തിനും ഇതിനകം തന്നെ സ്വന്തം വിലാസമുണ്ട്.',
  'server.publicationForeignNamespace':
    '{namespace} ഈ ആർക്കൈവിന്റെ നെയിംസ്‌പേസ് അല്ല. ഈ ആർക്കൈവിന്റെ /vocab/ വിലാസത്തിന് കീഴിലോ, w3id.org-ലോ, purl.org-ലോ ഉള്ള ഒരു പദാവലി പ്രസിദ്ധീകരിക്കുക.',
  'server.publicationSlugInvalid':
    'ഒരു വിലാസപ്പേര് ചെറിയ അക്ഷരങ്ങൾ, അക്കങ്ങൾ, ഒരു ഹൈഫൻ എന്നിവ ഉപയോഗിക്കുന്നു, 64 അക്ഷരങ്ങൾ വരെ.',
  'server.publicationSlugTaken': '/vocab/{slug}/ എന്ന വിലാസം നേരത്തെ തന്നെ എടുത്തിട്ടുണ്ട്.',
  'server.publicationNamespaceNotPersistent':
    'ഈ ആർക്കൈവിന്റെ നെയിംസ്‌പേസ് അല്ലാത്ത മറ്റൊന്ന് w3id.org അല്ലെങ്കിൽ purl.org-ൽ ആയിരിക്കണം, കൂടാതെ / അല്ലെങ്കിൽ # എന്നതിൽ അവസാനിക്കണം.',
  'server.publicationAddressFixed':
    'ഒരു പ്രസിദ്ധീകരിച്ച പദപ്പട്ടികയോ പദാവലിയോ അത് ആദ്യം പ്രസിദ്ധീകരിച്ച വിലാസം നിലനിർത്തുന്നു.',
  'server.publicationTermNotPublished':
    '"{term}" ഇതുവരെ പ്രസിദ്ധീകരിച്ചിട്ടില്ല, അതിനാൽ അത് ഡിപ്രീക്കേറ്റ് ചെയ്യാൻ കഴിയില്ല. അത് നീക്കം ചെയ്യുക.',
  'server.publicationUnknownTerm': '"{term}" ഈ പട്ടികയുടെയോ പദാവലിയുടെയോ ഒരു പദമല്ല.',
  'server.publicationMatchInvalid':
    'ഒരു മാച്ച് (match) ഒരു അബ്സല്യൂട്ട് http അല്ലെങ്കിൽ https URI ആയിരിക്കണം.',
  'server.publicationMatchTermLists':
    'പദപ്പട്ടികയിലെ ലിങ്കുകൾ ഒരു അതോറിറ്റിയിലേക്ക് മാത്രമായിരിക്കണം.',
  'server.publicationTermIriInvalid': '{uri} എന്ന ടേം URI ഒരു സാധുവായ IRI അല്ല.',
};
