/**
 * The Tamil Publish panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `PublishMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PublishMessages } from '../index';

export const ta: PublishMessages = {
  'publish.heading': 'வெளியிடு',
  'publish.ledeTermList':
    'வெளியிடுதல் என்பது இந்தப் பட்டியல் மற்றும் அதன் ஒவ்வொரு சொல்லுக்கும் ஒரு நிரந்தர இணைய முகவரியை வழங்குகிறது, அதை மக்கள் மற்றும் நிரல்கள் வாசிக்க முடியும்.',
  'publish.ledeVocabulary':
    'வெளியிடுவது இந்தத் தரநிலை மற்றும் அதன் ஒவ்வொரு விதிமுறைக்கும் ஒரு நிரந்தர இணைய முகவரியை வழங்குகிறது, அதை மக்கள் மற்றும் நிரல்கள் வாசிக்க முடியும்.',
  'publish.loading': 'வெளியீட்டை ஏற்றுகிறது...',
  'publish.stateDraft': 'வரைவு. இன்னும் வெளியிடப்படவில்லை.',
  'publish.statePublished': 'வெளியிடப்பட்டது. பதிப்பு {version}, {date}.',
  'publish.publicUrl': 'பொது முகவரி',
  'publish.promise':
    'வெளியிட்ட பிறகு, சொற்களைச் சேர்க்கலாம் அல்லது நீக்கலாம், ஆனால் அவற்றை ஒருபோதும் நீக்கவோ அல்லது மறுபெயரிடவோ முடியாது, மேலும் முகவரி மாறாது. பிற காப்பகங்கள் ஏற்கனவே அவற்றோடு இணைக்கப்பட்டிருக்கலாம்.',
  'publish.slugLabel': 'முகவரிப் பெயர்',
  'publish.slugHint':
    'சிறிய எழுத்துக்கள், எண்கள் மற்றும் ஒற்றை ஹைபன்கள். இது {url} இல் வழங்கப்படுகிறது',
  'publish.slugInvalid':
    'சிறிய எழுத்துக்கள், எண்கள் மற்றும் ஒற்றை ஹைபன்களைப் பயன்படுத்தவும், அதிகபட்சம் 64 எழுத்துக்கள்.',
  'publish.namespaceLegend': 'சொற்களின் முகவரிகள்',
  'publish.namespaceArchive': 'இந்த காப்பகத்தின் முகவரி: {url}',
  'publish.namespacePersistent': 'w3id.org அல்லது purl.org இல் நிலையான முகவரி',
  'publish.namespaceWhy':
    'ஒரு நிலையான முகவரி ஒரு களத்திற்கு மாற்றப்பட்டாலும் நீடிக்கும்: சேவை அதை இங்கே திசைதிருப்புகிறது, எனவே உங்கள் சொற்களுக்கான இணைப்புகள் காப்பகம் மாறினாலும் தொடர்ந்து செயல்படும்.',
  'publish.namespaceLabel': 'நிலையான பெயர்வெளி',
  'publish.namespaceHint':
    'உதாரணமாக https://w3id.org/உங்கள்-காப்பகம்/இடங்கள்/. இதை / அல்லது # உடன் முடிக்கவும்.',
  'publish.namespaceInvalid':
    'w3id.org அல்லது purl.org இல் / அல்லது # உடன் முடிவடையும் முகவரியைப் பயன்படுத்தவும்.',
  'publish.redirectHelp':
    'பின்னர் இந்த w3id.org விதியைப் பயன்படுத்தி அதை இங்கே திசைதிருப்பச் சொல்லுங்கள்:',
  'publish.namespace': 'பெயர்வெளி',
  'publish.termsHeading': 'கலைச்சொற்கள்',
  'publish.columnTerm': 'கலைச்சொல்',
  'publish.columnUri': 'URI',
  'publish.columnState': 'நிலை',
  'publish.columnChange': 'மாற்றம்',
  'publish.termDraft': 'இன்னும் வெளியிடப்படவில்லை',
  'publish.termPublished': 'வெளியிடப்பட்டது',
  'publish.termDeprecated': 'காலாவதியானது',
  'publish.termDeprecationPending': 'அடுத்த பதிப்பிலிருந்து நீக்கப்பட்டது',
  'publish.deprecate': 'காலாவதியாக்கு',
  'publish.deprecateLabel': '{term} ஐ காலாவதியாக்கு',
  'publish.undeprecate': 'வைத்திரு',
  'publish.undeprecateLabel': '{term} ஐப் பயன்படுத்தவும்',
  'publish.links': 'இணைப்புகள்',
  'publish.linksLabel': '{term} மற்றும் ஓர் அதிகாரத்திற்கு இடையிலான இணைப்புகள்',
  'publish.exactMatch': 'அதே சொல் மற்ற இடங்களில் (exactMatch)',
  'publish.closeMatch': 'ஒரு நெருக்கமான சொல் மற்ற இடங்களில் (closeMatch)',
  'publish.linksHint': 'இந்த சொல்லின் முகவரி GeoNames அல்லது Wikidata போன்ற ஓர் அதிகார அமைப்பில்.',
  'publish.saveLinks': 'இணைப்புகளைச் சேமிக்கவும்',
  'publish.matchInvalid': 'http:// அல்லது https:// இல் தொடங்கும் ஓர் முகவரியைப் பயன்படுத்தவும்.',
  'publish.publish': 'வெளியிடவும்',
  'publish.publishVersion': 'பதிப்பு {version}-ஐ வெளியிடவும்',
  'publish.noChanges': 'பதிப்பு {version}-இல் எந்த மாற்றமும் இல்லை.',
  'publish.pending':
    'பதிப்பு {version}-இல் செய்யப்பட்ட மாற்றங்கள் இன்னும் பொதுப்படையவில்லை. அவற்றை வெளியிட, வெளியிடவும்.',
  'publish.published': 'பதிப்பு {version} வெளியிடப்பட்டது.',
  'publish.versionsHeading': 'பதிப்புகள்',
  'publish.versionLine': 'பதிப்பு {version}, {date}, {terms} விதிமுறைகள்',
  'publish.previewHeading': 'டர்டில் முறையில் முன்னோட்டம்',
  'publish.replacedBy': '“{term}” மூலம் மாற்றப்பட்டது',
};
