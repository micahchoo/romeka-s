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
  'server.settingNotCount':
    '{key} എന്ന ക്രമീകരണം പൂജ്യത്തിന് മുകളിലുള്ള ഒരു പൂർണ്ണസംഖ്യയോ, അല്ലെങ്കിൽ നള്ളോ ആയിരിക്കണം.',
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
  'server.importAdminsOnly':
    'ഒരു ആർക്കൈവ് ഇംപോർട്ട് ചെയ്യാൻ ഒരു ഗ്ലോബൽ അല്ലെങ്കിൽ സൈറ്റ് അഡ്മിനിസ്ട്രേറ്റർക്ക് മാത്രമേ സാധിക്കൂ.',
  'server.importNotFound': 'ഒരു ഇംപോർട്ടിനും {id} എന്ന ഐഡി ഇല്ല.',
  'server.importRequestUnreadable': 'അഭ്യർത്ഥന വായിക്കാൻ കഴിയില്ല: {reason}',
  'server.importPlanRefused': 'ഇംപോർട്ടിന്റെ ക്രമീകരണങ്ങൾ സംരക്ഷിക്കാൻ കഴിയില്ല: {reason}',
  'server.importStale':
    'നിങ്ങൾ ഇത് തുറന്ന ശേഷം മറ്റൊരാൾ ഈ ഇംപോർട്ട് സേവ് ചെയ്തിട്ടുണ്ട്. അവരുടെ മാറ്റങ്ങൾ കാണാൻ ഇത് വീണ്ടും ലോഡ് ചെയ്യുക.',
  'server.importPlanFrozen':
    'ഈ ഇംപോർട്ട് നേരത്തെ ആരംഭിച്ചതിനാൽ, അതിന്റെ ക്രമീകരണങ്ങൾ മാറ്റാൻ കഴിയില്ല. ഇത് മാറ്റാൻ ഒരു പകർപ്പ് ഉണ്ടാക്കുക.',
  'server.importV1Plan':
    'ഈ ഇംപോർട്ട് പഴയ ഇംപോർട്ടർ ചെയ്തതാണ്, അതിനാൽ ഇത് ഇവിടെ തുറക്കാനോ പകർപ്പെടുക്കാനോ കഴിയില്ല.',
  'server.importTypeExists':
    '"{label}" എന്ന പേരിൽ ഒരു ഇനം തരം Omeka-യിൽ നേരത്തെ തന്നെയുണ്ട്. പുതിയ തരത്തിന് മറ്റൊരു പേര് തിരഞ്ഞെടുക്കുക.',
  'server.importDeleteNotDraft':
    'ഇതുവരെ ചേർക്കാത്ത ഒരു ഇംപോർട്ട് മാത്രമേ ഇല്ലാതാക്കാൻ കഴിയൂ. പ്രവർത്തിച്ച ഒരു ഇംപോർട്ട് മുൻ ഇംപോർട്ടുകളിൽ തുടരും.',
  'server.importChanged':
    'അവസാന പരിശോധനയ്ക്ക് ശേഷം ഫയലുകളിലോ ഇംപോർട്ടിന്റെ ക്രമീകരണങ്ങളിലോ മാറ്റം വന്നിട്ടുണ്ട്. ഇത് വീണ്ടും പ്രവർത്തിപ്പിക്കുന്നതിന് മുമ്പ് ഇംപോർട്ട് പരിശോധിക്കുക.',
  'server.importBlocked':
    'ഈ ഇംപോർട്ട് പ്രവർത്തിക്കുന്നതിന് മുമ്പ് പരിശോധന ലിസ്റ്റ് ചെയ്യുന്ന പ്രശ്നങ്ങൾ പരിഹരിക്കണം.',
  'server.importNothingToRun': 'ഈ ഇംപോർട്ടിൽ എഴുതാൻ ഒന്നുമില്ല.',
  'server.importConfirmDelete':
    'ഈ ഇംപോർട്ട് {count} ഇനങ്ങൾ, മീഡിയ, ഇനം സെറ്റുകൾ എന്നിവ ഇല്ലാതാക്കും, ഇത് അഴിച്ചെടുക്കാൻ കഴിയില്ല. ഇത് പ്രവർത്തിപ്പിക്കാൻ ആ സംഖ്യ സ്ഥിരീകരിക്കുക.',
  'server.importBusy':
    'മറ്റൊരു ഇംപോർട്ട് പ്രവർത്തിക്കുന്നു അല്ലെങ്കിൽ അഴിച്ചെടുക്കുന്നു. അത് കഴിയുമ്പോൾ വീണ്ടും ശ്രമിക്കുക.',
  'server.importCannotRun':
    'ഇതുവരെ ചേർക്കാത്തതോ, അവസാനം വരെ എത്താതെ നിന്നതോ, അല്ലെങ്കിൽ പരാജയങ്ങളോടെ പൂർത്തിയായതോ ആയ ഇംപോർട്ടുകൾ മാത്രമേ പ്രവർത്തിപ്പിക്കാൻ കഴിയൂ.',
  'server.importCannotUndo':
    'പൂർത്തിയായതോ, അവസാനം വരെ എത്താതെ നിന്നതോ ആയ ഇംപോർട്ടുകൾ മാത്രമേ അഴിച്ചെടുക്കാൻ കഴിയൂ.',
  'server.importNotRunning': 'ഈ ഇംപോർട്ട് പ്രവർത്തിക്കുന്നില്ല.',
  'server.importNoRoot': '"{root}" എന്ന് പേരുള്ള ആർക്കൈവ് ഫോൾഡർ ഇല്ല.',
  'server.importOutsideRoot': '"{path}" എന്ന ഫോൾഡർ ആർക്കൈവ് ഫോൾഡറിന് പുറത്താണ്.',
  'server.importFolderMissing': 'ആർക്കൈവ് ഫോൾഡറിൽ "{path}" എന്ന ഫോൾഡർ ഇല്ല.',
  'server.importNotAFolder': '"{path}" ഒരു ഫോൾഡറിന് പകരം ഒരു ഫയലാണ്.',
  'server.importScanBusy':
    'ഈ ഇംപോർട്ട് പ്രവർത്തിപ്പിക്കുന്നു അല്ലെങ്കിൽ അഴിച്ചെടുക്കുന്നു. അത് കഴിയുമ്പോൾ ഫയലുകൾ വീണ്ടും പരിശോധിക്കുക.',
  'server.importNoLeftFile':
    'ഈ ഇംപോർട്ടിൽ ഇതുവരെ ഒഴിവാക്കിയ ഫയലുകളുടെ പട്ടികയില്ല. ഒരു റൺ കഴിയുമ്പോൾ അത് എഴുതപ്പെടുന്നു.',
  'server.importJobNotStarted': 'ഇംപോർട്ട് തുടങ്ങാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importListColumnUnknown':
    'പട്ടികയിൽ "{column}" എന്ന കോളം ഉണ്ടാകാൻ പാടില്ല. ഒരു പ്രോപ്പർട്ടി, ഐറ്റം സെറ്റുകൾ, ഐറ്റം ടൈപ്പ് അല്ലെങ്കിൽ വിസിബിലിറ്റി തിരഞ്ഞെടുക്കുക.',
  'server.importListOneFile': 'ഫോമിൽ ഒരു ലിസ്റ്റ് ഫയൽ അയയ്ക്കുക.',
  'server.importListType':
    '"{name}" ഒരു .csv, .tsv, .xlsx, .xls അല്ലെങ്കിൽ .ods ഫയൽ അല്ല. ഈ പട്ടിക ഇവയിലൊന്നായി സേവ് ചെയ്ത് വീണ്ടും അയക്കുക.',
  'server.importListTooLarge': 'ലിസ്റ്റ് 50 MB-യിൽ വലുതാണ്.',
  'server.importListUnread': 'ലിസ്റ്റ് വായിക്കാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importListSeriesNeeded':
    'ഈ പ്ലാനിൽ നിരവധി സീരീസുകൾ ഉണ്ട്. ലിസ്റ്റ് ഏത് സീരീസിനുള്ളതാണെന്ന് തിരഞ്ഞെടുക്കുക.',
  'server.importListSeriesUnknown': 'ഈ പ്ലാനിൽ "{series}" സീരീസ് ഇല്ല.',
  'server.importListNoIdentifier':
    'ലിസ്റ്റിൽ ഐഡന്റിഫയർ കോളം ഇല്ല, അതിനാൽ അതിലെ വരികൾക്ക് അവയുടെ ഐറ്റങ്ങൾ കണ്ടെത്താൻ കഴിയില്ല. നിങ്ങൾ ഡൗൺലോഡ് ചെയ്ത ലിസ്റ്റിന്റെ ഐഡന്റിഫയർ കോളം നിലനിർത്തുക.',
  'server.importPairsColumns':
    'പട്ടികയിൽ Row കോളവും Files കോളവും വേണം. നിങ്ങൾ ഡൗൺലോഡ് ചെയ്ത പട്ടികയിലെ രണ്ട് കോളങ്ങളും അതുപോലെ നിലനിർത്തുക.',
  'server.importFileUnreadable': '"{path}" എന്ന ഫയൽ വായിക്കാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importFileRefused': '"{path}" എന്ന ഫയൽ ചേർക്കാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importFileNotStored': '"{path}" എന്ന ഫയൽ സേവ് ചെയ്യാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importSourceRefused': '"{source}" ചേർക്കാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importMediaNoSource': 'ഈ മീഡിയയിൽ ചേർക്കാൻ ഫയലോ വിലാസമോ നൽകിയിട്ടില്ല.',
  'server.importMediaNoItem': 'ഈ മീഡിയയുടെ ഐറ്റം ചേർക്കാത്തതിനാൽ, മീഡിയയും ചേർത്തില്ല.',
  'server.importResourceGone':
    'ഈ റെക്കോർഡ് മാറ്റുന്ന ഐറ്റം, മീഡിയ അല്ലെങ്കിൽ ഐറ്റം സെറ്റ് ഇപ്പോൾ Omeka-യിൽ ഇല്ല.',
  'server.importRecordNotSaved': 'ഈ റെക്കോർഡ് സേവ് ചെയ്യാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importLinksNotSaved': 'ഈ റെക്കോർഡിന്റെ ലിങ്കുകൾ സേവ് ചെയ്യാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.importRecordNotDeleted': 'ഇത് ഡിലീറ്റ് ചെയ്തില്ല: {reason}',
  'server.importDeleteNotUndone':
    'ഐറ്റങ്ങൾ, മീഡിയ അല്ലെങ്കിൽ ഐറ്റം സെറ്റുകൾ ഡിലീറ്റ് ചെയ്ത ഒരു ഇംപോർട്ട് അൺഡൂ ചെയ്യാൻ കഴിയില്ല.',
  'server.importSitesNotJoined':
    'ഈ ഐറ്റം ചേർത്തു, പക്ഷേ അതിനെ അതിന്റെ സൈറ്റുകളിൽ ചേർത്തില്ല: {reason}',
  'server.importUndoEdited': 'ഇംപോർട്ട് ഇത് എഴുതിയതിന് ശേഷം ഇത് എഡിറ്റ് ചെയ്തിട്ടുണ്ട്.',
  'server.importUndoLinked':
    'ഈ ഇംപോർട്ടിന് പുറത്തുള്ള ഒരു ഐറ്റം, മീഡിയ അല്ലെങ്കിൽ ഐറ്റം സെറ്റ് ഇതിലേക്ക് ലിങ്ക് ചെയ്യുന്നു.',
  'server.importUndoMediaAdded': 'ഈ ഐറ്റത്തിൽ ഇംപോർട്ടിന് പുറത്തുനിന്ന് മീഡിയ ചേർത്തു.',
  'server.importUndoMembersAdded':
    'ഈ ഇംപോർട്ടിന് പുറത്തുള്ള ഐറ്റങ്ങൾ ഈ ഐറ്റം സെറ്റിൽ ഉൾപ്പെടുത്തി.',
  'server.importUndoLaterImport': 'ഈ ഇംപോർട്ടിന് ശേഷം ഇംപോർട്ട് {import} ഇത് മാറ്റി.',
  'server.importUndoInUse':
    'ഈ ഇംപോർട്ടിന് പുറത്തുള്ള ഐറ്റങ്ങൾ, മീഡിയ, ഐറ്റം സെറ്റുകൾ അല്ലെങ്കിൽ ഐറ്റം ടൈപ്പുകൾ ഇത് ഉപയോഗിക്കുന്നു.',
  'server.importUndoItemKept': 'ഈ മീഡിയയുടെ ഐറ്റം നിലനിർത്തുന്നു, അതിനാൽ മീഡിയയും നിലനിർത്തുന്നു.',
  'server.importUndoMemberKept':
    'ഈ ഐറ്റം സെറ്റിലെ ഒരു ഐറ്റം നിലനിർത്തുന്നു, അതിനാൽ ഐറ്റം സെറ്റും നിലനിർത്തുന്നു.',
  'server.importUndoStopped': 'അൺഡൂ അവസാനിക്കുന്നതിന് മുമ്പ് നിന്നു: {reason}',
  'server.importUploadOwnerOnly':
    'ഈ ഇംപോർട്ട് തുടങ്ങിയ വ്യക്തിക്കോ, അല്ലെങ്കിൽ ഒരു ഗ്ലോബൽ അഡ്മിനിസ്ട്രേറ്റർക്കോ മാത്രമേ ഇതിലേക്ക് ഫയലുകൾ ചേർക്കാൻ കഴിയൂ.',
  'server.importUploadNotDraft':
    'ഇംപോർട്ട് തുടങ്ങുന്നതിന് മുമ്പ് മാത്രമേ ഫയലുകൾ ചേർക്കാൻ കഴിയൂ. കൂടുതൽ ഫയലുകൾ ചേർക്കാൻ, ഈ ഇംപോർട്ടിന്റെ ഒരു കോപ്പി ഉണ്ടാക്കുക.',
  'server.importUploadPath': '"{path}" എന്ന ഫയൽനാമം ഉപയോഗിക്കാൻ കഴിയില്ല: {reason}.',
  'server.importUploadTooLarge':
    'ഈ ഫയൽ {limit} ബൈറ്റിനേക്കാൾ വലുതാണ്, ഒരു ഇംപോർട്ടിന് എടുക്കുന്ന ഏറ്റവും വലിയ ഫയൽ ഇതാണ്.',
  'server.importUploadFull':
    'ഈ ഇംപോർട്ടിലെ ഫയലുകൾ {limit} ബൈറ്റിൽ കൂടുതൽ വരും, ഒരു ഇംപോർട്ടിൽ ഉൾക്കൊള്ളാവുന്ന ഏറ്റവും വലിയ ഫയൽ ഇതാണ്. ചില ഫയലുകൾ നീക്കം ചെയ്യുക, അല്ലെങ്കിൽ അവയുടെ കൈമാറ്റം പൂർത്തിയാകാൻ അനുവദിക്കുക, എന്നിട്ട് വീണ്ടും ശ്രമിക്കുക.',
  'server.importUploadChunkTooLarge':
    'ഈ അഭ്യർത്ഥന {limit} ബൈറ്റിൽ കൂടുതൽ വഹിക്കുന്നു. ഫയൽ ചെറിയ ഭാഗങ്ങളായി അയയ്ക്കുക.',
  'server.importUploadInProgress':
    '"{path}" ഇപ്പോഴും അതിന്റെ കൈമാറ്റത്തിലാണ്. അത് വീണ്ടും തുടങ്ങുന്നതിന് പകരം ആ അപ്‌ലോഡ് തുടരുക.',
  'server.importUploadNotFound': 'ഈ ഇംപോർട്ടിൽ "{upload}" എന്ന ഐഡിയുള്ള അപ്‌ലോഡ് ഇല്ല.',
  'server.importUploadOffset':
    'സെർവർ ഈ ഫയലിന്റെ {received} ബൈറ്റുകൾ സ്വീകരിച്ചു. അവയ്ക്ക് ശേഷമുള്ള ബൈറ്റുകൾ അയയ്ക്കുക.',
  'server.importUploadPastLength': 'ഈ ബൈറ്റുകൾ ഈ അപ്‌ലോഡിന്റെ പ്രഖ്യാപിത വലുപ്പത്തെ മറികടക്കുന്നു.',
  'server.importUploadLength': 'ഫയലിന്റെ വലുപ്പം ബൈറ്റുകളിൽ അപ്‌ലോഡ്-ലെങ്ത് ആയി അയയ്ക്കുക.',
  'server.importUploadMetadata':
    'ഫയലിന്റെ പേര് "filename" എന്ന് നൽകിക്കൊണ്ട്, അതിന്റെ പേര് ബേസ്64 രൂപത്തിൽ അപ്‌ലോഡ്-മെറ്റാഡാറ്റ ആയി അയയ്ക്കുക.',
  'server.importUploadOffsetMissing':
    'ഇതിനകം അയച്ച ബൈറ്റുകളുടെ എണ്ണം അപ്‌ലോഡ്-ഓഫ്‌സെറ്റ് ആയി അയയ്ക്കുക.',
  'server.importUploadContentType':
    'Content-Type application/offset+octet-stream ആയ ബൈറ്റുകൾ അയയ്ക്കുക.',
  'server.importUploadVersion':
    'ഈ സെർവർ റീസമ്മബിൾ അപ്‌ലോഡ് പ്രോട്ടോക്കോളിന്റെ 1.0.0 പതിപ്പ് സംസാരിക്കുന്നു. Tus-Resumable: 1.0.0 അയയ്ക്കുക.',
  'server.importUploadInterrupted':
    'അഭ്യർത്ഥന അവസാനിക്കുന്നതിന് മുമ്പ് ബൈറ്റുകൾ വരുന്നത് നിന്നു, അതിനാൽ അവയൊന്നും സംരക്ഷിക്കപ്പെട്ടില്ല. ഓഫ്‌സെറ്റ് ചോദിക്കുക, അവ വീണ്ടും അയയ്ക്കുക.',
  'server.importZipEscapes':
    '"{name}" എന്ന സിപ്പ് ഫയൽ അതിന്റെ ഫോൾഡറിന് പുറത്ത് വരും, അതിനാൽ അത് ഒഴിവാക്കി.',
  'server.importZipLink': '"{name}" എന്ന സിപ്പ് ഫയൽ ഒരു ലിങ്ക് ആണ്, അതിനാൽ അത് ഒഴിവാക്കി.',
  'server.importZipUnusable':
    '"{name}" എന്ന സിപ്പ് ഫയലിന് ഉപയോഗിക്കാൻ കഴിയാത്ത ഒരു പേരുണ്ട് ({reason}), അതിനാൽ അത് ഒഴിവാക്കി.',
  'server.importZipUnsupported':
    'സിപ്പിലെ "{name}" എൻക്രിപ്റ്റ് ചെയ്തതാണ്, അല്ലെങ്കിൽ റോംക എസിന് തുറക്കാൻ കഴിയാത്ത രീതിയിൽ പാക്ക് ചെയ്തതാണ്, അതിനാൽ അത് ഒഴിവാക്കി.',
  'server.importZipRenamed':
    '"{name}" എന്ന സിപ്പ് ഫയലിന് വലിയക്ഷരങ്ങൾ അവഗണിച്ചാൽ നേരത്തെയുള്ള ഒരു ഫയലിന്റെ പേരാണ്, അതിനാൽ അത് "{renamed}" ആയി ചേർത്തു.',
  'server.importZipBomb':
    'സിപ്പ് നിരസിക്കപ്പെട്ടു: "{name}" അൺപാക്ക് ചെയ്യുമ്പോൾ അതിന്റെ പാക്ക് ചെയ്ത വലുപ്പത്തിന്റെ 100 മടങ്ങിലധികം വരുന്നു. ഇതിൽ നിന്ന് ഒന്നും ചേർത്തില്ല.',
  'server.importZipFull':
    'സിപ്പ് നിരസിക്കപ്പെട്ടു: അതിന്റെ ഫയലുകൾ {limit} ബൈറ്റുകൾ കവിയും, ഒരു ഇംപോർട്ടിൽ പരമാവധി ഉള്ളത് ഇതാണ്. നിന്ന് ഒന്നും ചേർത്തില്ല.',
  'server.importZipTooMany':
    'സിപ്പ് നിരസിക്കപ്പെട്ടു: ഇതിൽ {count} ഫയലുകൾ ഉണ്ട്, 100,000-ൽ കൂടുതൽ. നിന്ന് ഒന്നും ചേർത്തില്ല.',
  'server.importZipFileTooLarge':
    'സിപ്പ് നിരസിക്കപ്പെട്ടു: "{name}" {limit} ബൈറ്റുകളേക്കാൾ വലുതാണ്, ഒരു ഇംപോർട്ടിൽ എടുക്കുന്ന ഏറ്റവും വലിയ ഫയൽ ഇതാണ്. നിന്ന് ഒന്നും ചേർത്തില്ല.',
  'server.importZipUnreadable': 'സിപ്പ് വായിക്കാൻ കഴിഞ്ഞില്ല ({reason}). നിന്ന് ഒന്നും ചേർത്തില്ല.',
  'server.importZipUnpacked': 'സിപ്പ് അൺപാക്ക് ചെയ്തു: {count} ഫയലുകൾ ചേർത്തു.',
  'server.importSweepDone':
    '{imports} ഇംപോർട്ടുകളിലെ ഫയലുകൾ നീക്കം ചെയ്തു, {uploads} പൂർത്തിയാകാത്ത അപ്‌ലോഡുകൾ.',
  'server.importAccessFailed':
    '{media}-യ്ക്ക് വേണ്ടി {reason} കാരണം പ്ലേ ചെയ്യാവുന്ന കോപ്പി ഒന്നും ക്യൂവിൽ ഇല്ല.',
  'server.importAccessUnread':
    'ഈ ഇംപോർട്ടിനായി {reason} കാരണം പ്ലേ ചെയ്യാവുന്ന കോപ്പികൾ ഒന്നും ക്യൂവിൽ ഇല്ല.',
  'server.mediaUploadNotFound': 'ഈ അക്കൗണ്ടിൽ "{upload}" ഐഡിയുള്ള സ്റ്റേജ്ഡ് അപ്‌ലോഡ് ഇല്ല.',
  'server.mediaUploadBusy':
    'ഈ അപ്‌ലോഡ് "{upload}" ഇപ്പോൾ ചേർക്കുന്നു. അത് കഴിഞ്ഞ ശേഷം വീണ്ടും ശ്രമിക്കുക.',
  'server.mediaUploadIncomplete':
    'ഈ അപ്‌ലോഡിൽ അതിന്റെ {length} ബൈറ്റുകളിൽ {received} ഉണ്ട്. ഇത് ചേർക്കുന്നതിന് മുമ്പ് ബാക്കി ഭാഗം അയക്കുക.',
  'server.mediaUploadIngester': 'സ്റ്റേജ്ഡ് അപ്‌ലോഡ് അപ്‌ലോഡ് ഇൻഗെസ്റ്റർ ചേർക്കുന്നു.',
  'server.noDerivativeToRetry': 'ഈ മീഡിയയ്ക്ക് വീണ്ടും നിർമ്മിക്കാൻ പരാജയപ്പെട്ട പകർപ്പുകളില്ല.',
  'server.mediaUploadInline':
    'ഒരു സ്റ്റേജ്ഡ് അപ്‌ലോഡ് അതിന്റെ സ്വന്തം മീഡിയ ക്രിയേറ്റ് ചേർക്കുന്നു, ഒരു ഐറ്റത്തിന് ഉള്ളിലല്ല.',
  'server.uploadTooLarge':
    'ഈ ഫയൽ അതിന്റെ {limit} ബൈറ്റുകളേക്കാൾ വലുതാണ്, ഒരു മീഡിയ അപ്‌ലോഡിൽ എടുക്കുന്ന ഏറ്റവും വലിയ ഫയൽ ഇതാണ്.',
  'server.mediaLinkInvalid': '"{url}" എന്ന ലിങ്ക് http അല്ലെങ്കിൽ https വിലാസമല്ല.',
  'server.mediaLinkUnreadable': '"{url}" എന്ന ലിങ്ക് വായിക്കാൻ കഴിഞ്ഞില്ല: {reason}',
  'server.mediaLinkStatus': '"{url}" എന്ന ലിങ്ക് {status} {reason} മറുപടി നൽകി.',
  'server.uploadItem': 'ഒരു അപ്‌ലോഡിന്റെ ഐറ്റം ഐഡി നിലവിലുള്ള ഒരു ഐറ്റത്തിന്റെ പേര് ആയിരിക്കണം.',
  'server.mediaUploadSweepDone':
    '{count} സ്റ്റേജ്ഡ് അപ്‌ലോഡുകൾ അവയുടെ സമയം കഴിഞ്ഞപ്പോൾ നീക്കം ചെയ്തു.',
};
