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
  'server.siteItemsOneSelector': '"query", "items" അല്ലെങ്കിൽ "item_sets" എന്നിവയിൽ ഒന്ന് മാത്രം',
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
  'server.customCssRefusal': 'കസ്റ്റം CSS: {refusal}',
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
};
