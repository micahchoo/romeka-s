/**
 * The Malayalam Standards and Fields page messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Malayalam. Correct
 * a string here; a later run keeps it. It is typed `StandardsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { StandardsMessages } from '../index';

export const ml: StandardsMessages = {
  'standards.lede':
    'നിങ്ങളുടെ ഫീൽഡുകളും ഐറ്റം ടൈപ്പുകളും സേവ് ചെയ്തിരിക്കുന്ന പങ്കിട്ട പദാവലികൾ. ഒരു ശേഖരം കാറ്റലോഗ് ചെയ്യുന്നതിന് നിങ്ങൾ അവ മാറ്റേണ്ടതില്ല.',
  'standards.define': '{term} എന്നാൽ എന്ത്?',
  'standards.import': 'ഒരു പദാവലി ഇറക്കുമതി ചെയ്യുക',
  'import.heading': 'ഒരു പദാവലി ഇറക്കുമതി ചെയ്യുക',
  'import.lede':
    'RDF ഫയലിൽ നിന്നോ വെബ് വിലാസത്തിൽ നിന്നോ പ്രസിദ്ധീകരിച്ച ഒരു പദാവലി കൊണ്ടുവരിക. ഇതിലെ ക്ലാസുകളും പ്രോപ്പർട്ടികളും നിങ്ങളുടെ ഫീൽഡുകൾക്കും ഐറ്റം ടൈപ്പുകൾക്കും ലഭ്യമാകും.',
  'import.submit': 'പദാവലി ഇറക്കുമതി ചെയ്യുക',
  'import.working': 'പദാവലി വായിക്കുന്നു…',
  'source.legend': 'പദാവലി എവിടെ നിന്ന് വരുന്നു',
  'source.upload': 'ഈ കമ്പ്യൂട്ടറിലെ ഒരു ഫയൽ',
  'source.url': 'ഒരു വെബ് വിലാസം',
  'source.file': 'പദാവലി ഫയൽ',
  'source.urlLabel': 'ഫയലിന്റെ വെബ് വിലാസം',
  'source.format': 'ഫയൽ ഫോർമാറ്റ്',
  'source.formatHelp':
    'ഇംപോർട്ട് ഫോർമാറ്റ് പറയാൻ കഴിയില്ലെങ്കിൽ, ഈ “Detect automatically” എന്നത് നിലനിർത്തുക.',
  'format.guess': 'സ്വയം കണ്ടെത്തുക',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'Turtle അല്ലെങ്കിൽ Notation3 (.ttl, .n3)',
  'format.ntriples': 'N-Triples (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'പദാവലിയെക്കുറിച്ച്',
  'about.namespace': 'നെയിംസ്പേസ് URI',
  'about.namespaceHelp':
    'പദാവലിയുടെ ഓരോ ക്ലാസ്സും പ്രോപ്പർട്ടിയും തുടങ്ങുന്ന വെബ് വിലാസം, ഉദാഹരണത്തിന് http://xmlns.com/foaf/0.1/. ഇതിന് താഴെയുള്ള പദങ്ങൾ മാത്രമേ ഇംപോർട്ട് ചെയ്യൂ. ഇത് സാധാരണ / അല്ലെങ്കിൽ # -ൽ അവസാനിക്കും.',
  'about.prefix': 'പ്രിഫിക്സ്',
  'about.prefixHelp':
    'ഓരോ പദത്തിന് മുൻപിലും എഴുതുന്ന ചെറിയ പേര്, ഉദാഹരണത്തിന് foaf:name. അക്ഷരങ്ങളും അക്കങ്ങളും ഹൈഫനുകളും അണ്ടർസ്കോറുകളും ഉപയോഗിക്കാം.',
  'about.label': 'ലേബൽ',
  'about.labelHelp': 'ഈ പദാവലിക്ക് ആളുകൾ കാണുന്ന പേര്.',
  'about.comment': 'വിവരണം (ഓപ്ഷണൽ)',
  'vocabularies.advancedSummary': 'അഡ്വാൻസ്ഡ്',
  'advanced.lang': 'മുൻഗണനയുള്ള ഭാഷ',
  'advanced.langHelp':
    'ഇംഗ്ലീഷ് അല്ലെങ്കിൽ ഫ്രഞ്ച് പോലുള്ള ഒരു ലാംഗ്വേജ് ടാഗ്. ആ ഭാഷയിലുള്ള ലേബലുകളും കമന്റുകളും നിലനിർത്തും. ഒന്നുമില്ലെങ്കിൽ, അല്ലെങ്കിൽ ആ പദത്തിൽ ഇല്ലാത്തപ്പോൾ, ഫയലിലെ ആദ്യത്തേത് നിലനിർത്തും.',
  'advanced.label': 'ലേബൽ പ്രോപ്പർട്ടി',
  'advanced.labelHelp':
    'അസാധാരണമായ പ്രോപ്പർട്ടി ഉപയോഗിച്ച് പദങ്ങളെ നാമകരണം ചെയ്യുന്ന വൊക്കാബുലറിയിൽ മാത്രം. ആംഗിൾ ബ്രാക്കറ്റിൽ പൂർണ്ണമായ പ്രോപ്പർട്ടി URI എഴുതുക, ഉദാഹരണത്തിന് <http://schema.org/name>. ഒന്നുമില്ലെങ്കിൽ, skos:prefLabel, rdfs:label, foaf:name, rss:title, dc:title എന്നിവ ക്രമമായി പരീക്ഷിക്കും.',
  'advanced.comment': 'കമന്റ് പ്രോപ്പർട്ടി',
  'advanced.commentHelp': 'ഓരോ പദത്തിന്റെയും വിവരണം. ഒന്നുമില്ലെങ്കിൽ, rdfs:comment വായിക്കും.',
  'error.noFile': 'ഇംപോർട്ട് ചെയ്യാൻ ഒരു ഫയൽ തിരഞ്ഞെടുക്കുക.',
  'error.noUrl': 'ഫയലിന്റെ വെബ് അഡ്രസ് നൽകുക.',
  'update.toggle': 'ഒരു ഫയലിൽ നിന്നോ വെബ് അഡ്രസ്സിൽ നിന്നോ അപ്‌ഡേറ്റ് ചെയ്യുക',
  'update.lede':
    'ഈ വൊക്കാബുലറിയെ അതിന്റെ ഫയലിന്റെ പുതിയ കോപ്പിയുമായി താരതമ്യം ചെയ്യുക. സേവ് ചെയ്യുന്നതിന് മുമ്പ് എല്ലാ മാറ്റങ്ങളും കാണാം.',
  'update.compare': 'താരതമ്യം ചെയ്യുക',
  'update.comparing': 'താരതമ്യം ചെയ്യുന്നു…',
  'diff.review': 'ഇവ പ്രയോഗിക്കുന്നതിന് മുമ്പ് അവലോകനം ചെയ്യുക.',
  'diff.none': 'ഈ വൊക്കാബുലറി ഫയലുമായി പൊരുത്തപ്പെടുന്നു. മാറ്റങ്ങളൊന്നുമില്ല.',
  'diff.newClasses': 'പുതിയ ക്ലാസുകൾ ({count})',
  'diff.newProperties': 'പുതിയ പ്രോപ്പർട്ടികൾ ({count})',
  'diff.changed': 'മാറ്റിയ ലേബലുകളും കമന്റുകളും ({count})',
  'diff.removed': 'ഫയലിൽ നിന്ന് നീക്കം ചെയ്തു ({count})',
  'diff.removedNote':
    'ഇവ ആർക്കൈവിൽ തുടരും, അവ ഉപയോഗിക്കുന്ന വാല്യൂകളും ഐറ്റം ടൈപ്പുകളും അതും തുടരും. ഇനി ഇവ ആവശ്യമില്ലെങ്കിൽ അവയുടെ പേജിൽ നിന്ന് ഒന്ന് ഡിലീറ്റ് ചെയ്യുക.',
  'diff.term': 'പദം',
  'diff.label': 'ലേബൽ',
  'diff.comment': 'കമന്റ്',
  'diff.what': 'എന്ത് മാറ്റങ്ങൾ',
  'diff.now': 'ഇപ്പോൾ',
  'diff.file': 'ഫയലിൽ',
  'diff.blank': '(ഒന്നുമില്ല)',
  'diff.apply': 'ഈ മാറ്റങ്ങൾ പ്രയോഗിക്കുക',
  'diff.applying': 'പ്രയോഗിക്കുന്നു…',
  'diff.cancel': 'റദ്ദാക്കുക',
  'diff.applied': 'മാറ്റങ്ങൾ സംരക്ഷിക്കപ്പെട്ടു.',
};
