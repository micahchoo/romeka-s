/**
 * The Malayalam Publish panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Malayalam. Correct
 * a string here; a later run keeps it. It is typed `PublishMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PublishMessages } from '../index';

export const ml: PublishMessages = {
  'publish.heading': 'പ്രസിദ്ധീകരിക്കുക',
  'publish.ledeTermList':
    'പ്രസിദ്ധീകരിക്കുമ്പോൾ ഈ പട്ടികയ്ക്കും അതിലെ ഓരോ പദത്തിനും ആളുകൾക്കും പ്രോഗ്രാമുകൾക്കും വായിക്കാൻ കഴിയുന്ന സ്ഥിരമായ വെബ് വിലാസം ലഭിക്കുന്നു.',
  'publish.ledeVocabulary':
    'പ്രസിദ്ധീകരിക്കുമ്പോൾ ഈ പദാവലിക്കും അതിലെ ഓരോ പദത്തിനും ആളുകൾക്കും പ്രോഗ്രാമുകൾക്കും വായിക്കാൻ കഴിയുന്ന സ്ഥിരമായ വെബ് വിലാസം ലഭിക്കുന്നു.',
  'publish.loading': 'പ്രസിദ്ധീകരണം ലോഡ് ചെയ്യുന്നു...',
  'publish.stateDraft': 'കരട്. ഇതുവരെ പ്രസിദ്ധീകരിച്ചിട്ടില്ല.',
  'publish.statePublished': 'പ്രസിദ്ധീകരിച്ചു. പതിപ്പ് {version}, {date}.',
  'publish.publicUrl': 'പൊതു വിലാസം',
  'publish.promise':
    'പ്രസിദ്ധീകരിച്ചാൽ, പദങ്ങൾ ചേർക്കാനോ ഒഴിവാക്കാനോ കഴിയും, പക്ഷേ ഒരിക്കലും നീക്കം ചെയ്യാനോ പേര് മാറ്റാനോ കഴിയില്ല, വിലാസം മാറുകയുമില്ല. മറ്റ് ആർക്കൈവുകൾ ഇതിനകം അവയുമായി ബന്ധിപ്പിച്ചിട്ടുണ്ടാകാം.',
  'publish.slugLabel': 'വിലാസനാമം',
  'publish.slugHint': 'ചെറിയ അക്ഷരങ്ങൾ, അക്കങ്ങൾ, ഒരു ഹൈഫൻ. ഇത് {url}-ൽ ലഭ്യമാണ്',
  'publish.slugInvalid': 'ചെറിയ അക്ഷരങ്ങൾ, അക്കങ്ങൾ, ഒരു ഹൈഫൻ, 64 അക്ഷരങ്ങൾ വരെ',
  'publish.namespaceLegend': 'പദങ്ങളുടെ വിലാസങ്ങൾ',
  'publish.namespaceArchive': 'ഈ ആർക്കൈവിന്റെ വിലാസം: {url}',
  'publish.namespacePersistent': 'w3id.org അല്ലെങ്കിൽ purl.org-ൽ സ്ഥിരമായ വിലാസം',
  'publish.namespaceWhy':
    'മറ്റൊരു ഡൊമെയ്‌നിലേക്കുള്ള മാറ്റത്തിന് ശേഷവും നിലനിൽക്കുന്ന സ്ഥിരമായ വിലാസം: സർവീസ് ഇവിടെ തിരിച്ചുവിടുന്നു, അതിനാൽ നിങ്ങളുടെ പദങ്ങളുടെ ലിങ്കുകൾ പ്രവർത്തിച്ചുകൊണ്ടിരിക്കും.',
  'publish.namespaceLabel': 'സ്ഥിരമായ നെയിംസ്‌പേസ്',
  'publish.namespaceHint':
    'ഉദാഹരണത്തിന് https://w3id.org/your-archive/places/. ഇത് / അല്ലെങ്കിൽ # വെച്ച് അവസാനിപ്പിക്കുക.',
  'publish.namespaceInvalid':
    'w3id.org അല്ലെങ്കിൽ purl.org-ൽ / അല്ലെങ്കിൽ # വെച്ച് അവസാനിക്കുന്ന ഒരു വിലാസം ഉപയോഗിക്കുക.',
  'publish.redirectHelp':
    'തുടർന്ന് സർവീസ് ഇതിനെ ഇവിടെ തിരിച്ചുവിടാൻ ആവശ്യപ്പെടുക, ഉദാഹരണത്തിന് ഈ w3id.org നിയമം ഉപയോഗിച്ച്:',
  'publish.namespace': 'നെയിംസ്‌പേസ്',
  'publish.termsHeading': 'പദങ്ങൾ',
  'publish.columnTerm': 'പദം',
  'publish.columnUri': 'URI',
  'publish.columnState': 'അവസ്ഥ',
  'publish.columnChange': 'മാറ്റം',
  'publish.termDraft': 'പ്രസിദ്ധീകരിച്ചിട്ടില്ല',
  'publish.termPublished': 'പ്രസിദ്ധീകരിച്ചു',
  'publish.termDeprecated': 'കാലഹരണപ്പെട്ടത്',
  'publish.termDeprecationPending': 'അടുത്ത പതിപ്പിൽ നിന്ന് കാലഹരണപ്പെട്ടത്',
  'publish.deprecate': 'കാലഹരണപ്പെടുത്തുക',
  'publish.deprecateLabel': '{term} കാലഹരണപ്പെടുത്തുക',
  'publish.undeprecate': 'നിലനിർത്തുക',
  'publish.undeprecateLabel': '{term} ഉപയോഗത്തിൽ നിലനിർത്തുക',
  'publish.links': 'ലിങ്കുകൾ',
  'publish.linksLabel': '{term}-ന്റെ ലിങ്കുകൾ ഒരു അധികാരത്തിലേക്ക്',
  'publish.exactMatch': 'അതേ പദം മറ്റൊരിടത്ത് (exactMatch)',
  'publish.closeMatch': 'മറ്റൊരു പദത്തോട് അടുത്ത പദമുള്ള elsewhere (closeMatch)',
  'publish.linksHint': 'GeoNames അല്ലെങ്കിൽ Wikidata പോലുള്ള ഒരു അധികാരത്തിൽ ഈ പദത്തിന്റെ വിലാസം.',
  'publish.saveLinks': 'ലിങ്കുകൾ സംരക്ഷിക്കുക',
  'publish.matchInvalid': 'http:// അല്ലെങ്കിൽ https:// എന്ന് തുടങ്ങുന്ന ഒരു വിലാസം ഉപയോഗിക്കുക.',
  'publish.publish': 'പ്രസിദ്ധീകരിക്കുക',
  'publish.publishVersion': 'പതിപ്പ് {version} പ്രസിദ്ധീകരിക്കുക',
  'publish.noChanges': 'പതിപ്പ് {version}-ന് ശേഷം മാറ്റങ്ങളൊന്നുമില്ല.',
  'publish.pending':
    'പതിപ്പ് {version}-ൽ നിന്നുള്ള മാറ്റങ്ങൾ ഇതുവരെ പരസ്യമായിട്ടില്ല. അവ ചേർക്കാൻ പ്രസിദ്ധീകരിക്കുക.',
  'publish.published': 'പതിപ്പ് {version} പ്രസിദ്ധീകരിച്ചു.',
  'publish.versionsHeading': 'പതിപ്പുകൾ',
  'publish.versionLine': 'പതിപ്പ് {version}, {date}, {terms} നിബന്ധനകൾ',
  'publish.previewHeading': 'Turtle-ൽ പ്രിവ്യൂ',
  'publish.replacedBy': '“{term}” പകരമായി വന്നു',
};
