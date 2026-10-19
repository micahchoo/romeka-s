/**
 * The Tamil vocabulary import messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `ImportMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ImportMessages } from '../messages';

export const ta: ImportMessages = {
  'import.heading': 'ஒரு சொல்லகராதியை இறக்குமதி செய்க',
  'import.lede':
    'RDF கோப்பு அல்லது இணைய முகவரியிலிருந்து வெளியிடப்பட்ட ஒரு சொல்லகராதியை இறக்குமதி செய்க. அதன் வகுப்புகள் மற்றும் பண்புகள் உங்கள் புலங்கள் மற்றும் உருப்படி வகைகளுக்குக் கிடைக்கின்றன.',
  'import.submit': 'சொல்லகராதியை இறக்குமதி செய்க',
  'import.working': 'சொல்லகராதியை வாசிக்கிறது…',
  'source.legend': 'சொல்லகராதி எங்கிருந்து வருகிறது',
  'source.upload': 'இந்த கணினியில் உள்ள ஒரு கோப்பு',
  'source.url': 'ஒரு இணைய முகவரி',
  'source.file': 'சொல்லகராதி கோப்பு',
  'source.urlLabel': 'கோப்பின் இணைய முகவரி',
  'source.format': 'கோப்பு வடிவம்',
  'source.formatHelp':
    'இறக்குமதி செய்யும்போது அது வடிவத்தைக் கண்டறிய இயலவில்லை என்று கூறினால் ஒழிய, இதை “தானாகக் கண்டறியவும்” என்றே விட்டுவிடவும்.',
  'format.guess': 'தானாகக் கண்டறியவும்',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'டர்டில் அல்லது நோடேஷன்3 (.ttl, .n3)',
  'format.ntriples': 'என்-ட்ரிப்ளஸ்ஸ் (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'சொல்லகராதி பற்றியது',
  'about.namespace': 'நேம்ஸ்பேஸ் யுஆர்ஐ',
  'about.namespaceHelp':
    'சொல்லகராதியின் ஒவ்வொரு வகுப்பு மற்றும் பண்புக்கூறு தொடங்கும் வலை முகவரி, உதாரணமாக http://xmlns.com/foaf/0.1/. இது பொதுவாக / அல்லது # உடன் முடிவடையும்.',
  'about.prefix': 'முன்னொட்டு',
  'about.prefixHelp':
    'ஒவ்வொரு சொல்லுக்கு முன்னால் எழுதப்படும் குறுகிய பெயர், உதாரணமாக foaf:name. எழுத்துக்கள், எண்கள், ஹைபன்கள் மற்றும் அண்டர்ஸ்கோர்களைப் பயன்படுத்தவும்.',
  'about.label': 'லேபிள்',
  'about.labelHelp': 'சொல்லகராதிக்கு மக்கள் காணும் பெயர்.',
  'about.comment': 'விளக்கம் (விருப்பத்தேர்வு)',
  'advanced.summary': 'மேம்பட்ட',
  'advanced.lang': 'விருப்பமான மொழி',
  'advanced.langHelp':
    'ஆங்கிலம் அல்லது பிரஞ்சு போன்ற ஒரு மொழி குறியீடு. அந்த மொழியில் உள்ள லேபிள்கள் மற்றும் கருத்துகள் பாதுகாக்கப்படும். இது இல்லையெனில், அல்லது ஒரு சொல்லில் அது இல்லாதபோது, கோப்பில் உள்ள முதல் லேபிள் பயன்படுத்தப்படும்.',
  'advanced.label': 'லேபிள் பண்பு',
  'advanced.labelHelp':
    'வழக்கத்திற்கு மாறான பண்பு மூலம் தனது சொற்களை பெயரிடும் சொல்லகராதிக்கு மட்டுமே. கோண அடைப்புக்குறிக்குள் முழுமையான பண்பு URI ஐ எழுதவும், உதாரணமாக <http://schema.org/name>. இது இல்லையெனில், skos:prefLabel, rdfs:label, foaf:name, rss:title மற்றும் dc:title ஆகியவை வரிசையாக முயற்சிக்கப்படும்.',
  'advanced.comment': 'கருத்துப் பண்பு',
  'advanced.commentHelp': 'ஒவ்வொரு சொல்லின் விளக்கத்திற்கும், rdfs:comment வாசிக்கப்படுகிறது.',
  'error.noFile': 'இறக்குமதி செய்ய ஒரு கோப்பைத் தேர்ந்தெடுக்கவும்.',
  'error.noUrl': 'கோப்பின் இணைய முகவரியை உள்ளிடவும்.',
  'update.toggle': 'கோப்பு அல்லது இணைய முகவரியிலிருந்து புதுப்பிக்கவும்',
  'update.lede':
    'இந்த சொல்லகராதியை அதன் கோப்பின் புதிய நகலுடன் ஒப்பிடவும். எதையும் சேமிப்பதற்கு முன் ஒவ்வொரு மாற்றத்தையும் நீங்கள் காணலாம்.',
  'update.compare': 'ஒப்பிடுக',
  'update.comparing': 'ஒப்பிடுகிறது…',
  'diff.review': 'இந்த மாற்றங்களைச் செயல்படுத்துவதற்கு முன் அவற்றை ஆய்வு செய்யவும்.',
  'diff.none': 'இந்த சொல்லகராதி ஏற்கனவே கோப்புடன் பொருந்துகிறது. மாற்ற வேண்டியவை ஏதுமில்லை.',
  'diff.newClasses': 'புதிய வகுப்புகள் ({count})',
  'diff.newProperties': 'புதிய பண்புகள் ({count})',
  'diff.changed': 'மாற்றப்பட்ட லேபிள்கள் மற்றும் கருத்துகள் ({count})',
  'diff.removed': 'கோப்பில் இனி இல்லாதவை ({count})',
  'diff.removedNote':
    'இவை காப்பகத்தில் இருக்கும், மேலும் அவற்றைப் பயன்படுத்தும் மதிப்புகள் மற்றும் உருப்படி வகைகளும் இருக்கும். இனி அவற்றின் தேவை இல்லை என்றால், அவற்றின் பக்கத்திலிருந்து ஒன்றை நீக்கவும்.',
  'diff.term': 'சொல்',
  'diff.label': 'லேபிள்',
  'diff.comment': 'கருத்து',
  'diff.what': 'என்ன மாற்றங்கள்',
  'diff.now': 'இப்போது',
  'diff.file': 'கோப்பில்',
  'diff.blank': '(ஏதுமில்லை)',
  'diff.apply': 'இந்த மாற்றங்களைப் பயன்படுத்தவும்',
  'diff.applying': 'பயன்படுத்தப்படுகிறது…',
  'diff.cancel': 'ரத்து செய்',
  'diff.applied': 'மாற்றங்கள் சேமிக்கப்படுகின்றன.',
};
