/**
 * The Tamil Term lists messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `TermListMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { TermListMessages } from '../messages';

export const ta: TermListMessages = {
  'termLists.resource': 'கலைச்சொல் பட்டியல்கள்',
  'termLists.invalidId': '"{param}" என்பது கலைச்சொல் பட்டியல் அடையாள எண் அல்ல.',
  'termLists.intro':
    'ஒரு கலைச்சொல் பட்டியல் ஒரு புலத்தை நிலையான உள்ளீடுகளின் தொகுப்பிலிருந்து ஒரு தேர்வாக மாற்றுகிறது, எனவே அதே இடம் அல்லது பொருள் எப்போதும் ஒரே மாதிரியாகவே எழுதப்படுகிறது.',
  'termLists.add': 'கலைச்சொல் பட்டியலைச் சேர்க்க',
  'termLists.legend': 'கலைச்சொல் பட்டியல்',
  'termLists.kindLegend': 'இந்தப் பட்டியல் எவற்றைக் கொண்டுள்ளது',
  'termLists.kindTerms': 'சொற்களின் பட்டியல்',
  'termLists.kindItems': 'உருப்படிகளின் தொகுப்பிலிருந்து உருப்படிகள்',
  'termLists.kindUris': 'லேபிள்களுடன் கூடிய இணைப்புகள் (URIs)',
  'termLists.kindEmpty': 'இன்னும் எதுவும் இல்லை',
  'termLists.termsLabel': 'ஒரு வரிக்கு ஒரு சொல்',
  'termLists.urisLabel': 'ஒரு வரிக்கு ஒரு இணைப்பு: URI, ஒரு இடைவெளி, பின்னர் லேபிள்',
  'termLists.itemSetLabel': 'உருப்படிகளின் தொகுப்பு',
  'termLists.selectItemSet': 'ஒரு உருப்படத் தொகுப்பைத் தேர்ந்தெடுக்கவும்',
  'termLists.langLabel': 'சொற்களின் மொழி',
  'termLists.langHint':
    'விருப்பத்தேர்வு. பட்டியலில் இருந்து தேர்ந்தெடுக்கப்படும் ஒவ்வொரு சொல்லும் இந்த மொழி குறியீட்டைத் தாங்கி நிற்கும், உதாரணமாக en.',
  'termLists.errorNoEntries': 'குறைந்தபட்சம் ஒரு பதிவைச் சேர்க்கவும்.',
  'termLists.errorNoItemSet': 'ஒரு உருப்படத் தொகுப்பைத் தேர்ந்தெடுக்கவும்.',
  'termLists.columnKind': 'வைத்திருப்பவை',
  'termLists.columnEntries': 'பதிவுகள்',
  'termLists.columnUsage': 'இதைப் பயன்படுத்துபவர்கள்',
  'termLists.editLabel': 'இந்தச் சொல் பட்டியலைத் திருத்தவும்',
  'termLists.deleteLabel': 'இந்தச் சொல் பட்டியலை நீக்கவும்',
  'termLists.deleteNote':
    'ஒரு பட்டியலை நீக்குவது அதில் இருந்து தேர்ந்தெடுக்கப்பட்ட மதிப்புகளை நீக்கும். அவை அவற்றின் உருப்படங்களில் சாதாரண உரையாகவே இருக்கும்.',
  'termPicker.loading': 'பட்டியல் ஏற்றப்படுகிறது...',
  'termPicker.noMatch': '"{query}" உடன் பொருந்தக்கூடிய பதிவு எதுவும் இல்லை.',
  'termPicker.notInList':
    '"{value}" இந்த பட்டியலில் இல்லை. அதை மாற்ற ஒரு பதிவைத் தேர்ந்தெடுக்கவும்.',
  'termPicker.hint': '{list} பட்டியலை வடிகட்ட, பின்னர் ஒரு பதிவைத் தேர்ந்தெடுக்கவும்.',
  'termPicker.typeLabel': 'சொல் பட்டியல்: {list}',
  'tags.title': 'குறிச்சொற்கள்',
  'tags.intro':
    'விவரிக்கும்போது மக்கள் சேர்த்த கட்டற்ற சொற்கள். சிலரே மீண்டும் மீண்டும் வரும்போது, அவற்றை ஒரு பட்டியலாக மாற்றவும்.',
  'tags.empty':
    'இன்னும் குறிச்சொற்கள் இல்லை. ஒரு உருப்படி, உருப்படித் தொகுப்பு அல்லது ஊடகத்தின் குறிச்சொல் புலத்தில் அவற்றைச் சேர்க்கவும்.',
  'tags.legend': 'ஊக்குவிக்க வேண்டிய குறிச்சொற்கள்',
  'tags.listName': 'பட்டியல் பெயர்',
  'tags.listHint':
    'நீங்கள் குறிச்சொற்களைச் சேர்க்க விரும்பும் சொற்களின் புதிய பட்டியல், அல்லது சொற்களின் பட்டியலின் பெயர்.',
  'tags.promote': 'பட்டியலுக்கு ஊக்குவிக்கவும்',
  'tags.promoteTitle': '“{list}” க்கு ஊக்குவிக்கவும்',
  'tags.newList': 'இது ஒரு புதிய பட்டியலை உருவாக்குகிறது.',
  'tags.fields': 'பொருளின் பாடப்புலம் இந்தப் பட்டியலை வழங்கும்',
  'tags.fieldsSkipped': 'நீங்கள் திருத்த முடியாத, மாற்றாமல் அப்படியே விடப்பட்ட பொருளின் வகை',
  'tags.skipped': 'உங்களால் திருத்த முடியாத வளங்கள், ஆனால் அவற்றின் குறிச்சொற்களைத் தக்கவைப்பவை',
  'tags.promoted': '“{list}” க்கு ஊக்குவிக்கப்பட்டது. வளங்கள் மாற்றப்பட்டுள்ளன: {changed}.',
  'rewrite.changes': 'மாற்றப்படும் வளங்கள்',
  'rewrite.skipped': 'நீங்கள் திருத்த முடியாத, எழுத்துப்பிழையைத் தக்கவைத்துள்ள வளங்கள்',
  'rewrite.none': 'எதுவுமில்லை',
  'merge.title': 'ஒத்த எழுத்துப்பிழைகள்',
  'merge.intro':
    'இவை ஒரே மதிப்பைக் கொண்டுள்ளன. அவற்றை ஒன்றிணைக்கவும், இதன் மூலம் ஒரு தேடல் அனைத்து முடிவுகளையும் கண்டறியும்.',
  'merge.keep': '“{value}” ஐத் தக்கவைக்கவும்',
  'merge.usedBy': '{count} ஆல் பயன்படுத்தப்படுகிறது',
  'merge.into': '“{value}” உடன் ஒன்றிணைக்கவும்',
  'merge.confirm': 'ஒன்றிணைக்கவும்',
  'merge.different': 'அவை வேறுபட்டவை',
  'merge.merged': '“{value}” உடன் ஒன்றிணைக்கப்பட்டது. வளங்கள் மாற்றப்பட்டன: {changed}.',
  'merge.remembered': 'குறிக்கப்பட்டது. இந்த எழுத்துப்பிழைகள் மீண்டும் ஒன்றிணைக்கப்படாது.',
  'merge.deprecates':
    'இந்தப் பட்டியல் வெளியிடப்பட்டது, மேலும் ஒரு வெளியிடப்பட்ட சொல் அதில் இருந்து வெளியேறாது. {values} என்பது “{value}” மூலம் மாற்றப்படும் வரை, அதற்கான இணைப்புகள் தொடர்ந்து செயல்படும். இந்தப் பட்டியலை மீண்டும் வெளியிடவும்.',
  'termPicker.deprecated':
    '“{value}” இந்த பட்டியலில் காலாவதியானது. தற்போதைய பதிவைத் தேர்ந்தெடுக்கும் வரை இது இந்த உருப்படியில் இருக்கும்.',
};
