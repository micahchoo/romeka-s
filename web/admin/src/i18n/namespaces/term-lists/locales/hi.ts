/**
 * The Hindi Term lists messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `TermListMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { TermListMessages } from '../index';

export const hi: TermListMessages = {
  'termLists.resource': 'शब्द सूचियाँ',
  'termLists.invalidId': '"{param}" शब्द सूची की आईडी नहीं है।',
  'termLists.intro':
    'एक शब्द सूची किसी क्षेत्र या विषय को एक निश्चित सेट की प्रविष्टियों से विकल्प में बदल देती है, ताकि एक ही स्थान या विषय को हमेशा एक ही तरह से लिखा जाए।',
  'termLists.add': 'शब्द सूची जोड़ें',
  'termLists.legend': 'शब्द सूची',
  'termLists.kindLegend': 'सूची में क्या है',
  'termLists.kindTerms': 'शब्दों की सूची',
  'termLists.kindItems': 'आइटम सेट से आइटम',
  'termLists.kindUris': 'लेबल के साथ लिंक (URIs)',
  'termLists.kindEmpty': 'अभी कुछ नहीं',
  'termLists.termsLabel': 'प्रति पंक्ति एक शब्द',
  'termLists.urisLabel': 'लिंक, प्रति पंक्ति एक: URI, एक स्पेस, फिर लेबल',
  'termLists.itemSetLabel': 'आइटम सेट',
  'termLists.selectItemSet': 'आइटम सेट चुनें',
  'termLists.langLabel': 'शब्दों की भाषा',
  'termLists.langHint':
    'वैकल्पिक। सूची से चुने गए प्रत्येक शब्द में यह भाषा टैग होता है, उदाहरण के लिए en।',
  'termLists.errorNoEntries': 'कम से कम एक प्रविष्टि जोड़ें।',
  'termLists.errorNoItemSet': 'आइटम सेट चुनें।',
  'termLists.columnKind': 'होल्ड्स',
  'termLists.columnEntries': 'प्रविष्टियाँ',
  'termLists.columnUsage': 'इसका उपयोग करने वाले आइटम',
  'termLists.deleteLabel': 'इस शब्द सूची को हटाएँ',
  'termLists.deleteNote':
    'सूची को हटाने से उसमें से चुने गए मान हट जाते हैं। वे उनके आइटम पर सादे टेक्स्ट के रूप में बने रहते हैं।',
  'termPicker.loading': 'सूची लोड हो रही है...',
  'termPicker.noMatch': 'कोई प्रविष्टि "{query}" से मेल नहीं खाती।',
  'termPicker.notInList': '"{value}" इस सूची में नहीं है। इसे बदलने के लिए एक प्रविष्टि चुनें।',
  'termPicker.hint': '{list} सूची को फ़िल्टर करने के लिए टाइप करें, फिर एक प्रविष्टि चुनें।',
  'termPicker.typeLabel': 'शब्द सूची: {list}',
  'tags.title': 'टैग',
  'tags.intro':
    'वे शब्द जो लोगों ने वर्णन करते समय जोड़े। जब कुछ लोग बार-बार आते हैं, तो उन्हें एक सूची बनाएं।',
  'tags.empty': 'अभी तक कोई टैग नहीं है। आइटम, आइटम सेट या मीडिया के टैग फ़ील्ड में उन्हें जोड़ें।',
  'termLists.tagsLegend': 'बढ़ावा देने के लिए टैग',
  'tags.listName': 'सूची का नाम',
  'tags.listHint':
    'शब्दों की एक नई सूची, या शब्दों की एक सूची जो आपके पास है, जिसमें आप टैग जोड़ना चाहते हैं।',
  'tags.promote': 'सूची में बढ़ावा दें',
  'tags.promoteTitle': '“{list}” में बढ़ावा दें',
  'tags.newList': 'यह एक नई सूची बनाता है।',
  'tags.fields': 'आइटम के प्रकार जिनकी विषय फ़ील्ड सूची प्रदान करेगी',
  'tags.fieldsSkipped': 'आइटम के प्रकार जिन्हें आप संपादित नहीं कर सकते, जैसा वे हैं',
  'tags.skipped': 'वे संसाधन जिन्हें आप संपादित नहीं कर सकते, जो अपने टैग रखते हैं',
  'tags.promoted': '“{list}” में बढ़ावा दिया गया। संसाधन बदले: {changed}।',
  'rewrite.changes': 'संसाधन जो बदलते हैं',
  'rewrite.skipped': 'संसाधन जिन्हें आप संपादित नहीं कर सकते, जो उनकी वर्तनी बनाए रखते हैं',
  'rewrite.none': 'कोई नहीं',
  'merge.title': 'समान वर्तनी',
  'merge.intro': 'ये एक ही मान जैसे दिखते हैं। उन्हें इस तरह मिलाएँ कि एक खोज से वे सभी मिल जाएँ।',
  'merge.keep': '“{value}” रखें',
  'merge.usedBy': '{count} द्वारा उपयोग किया गया',
  'merge.into': '“{value}” में मिलाएँ',
  'merge.confirm': 'मर्ज',
  'merge.different': 'वे अलग-अलग हैं',
  'merge.merged': '“{value}” में मिला दिया गया। संसाधन बदल गए: {changed}।',
  'merge.remembered': 'नोट किया गया। इन वर्तनी को फिर से विलय के लिए पेश नहीं किया जाएगा।',
  'merge.deprecates':
    'यह सूची प्रकाशित है, और एक प्रकाशित शब्द कभी भी इसे नहीं छोड़ता है। {values} को अप्रचलित के रूप में रखा जाएगा, “{value}” द्वारा प्रतिस्थापित किया जाएगा, ताकि इसके लिंक काम करते रहें। इसे सार्वजनिक बनाने के लिए सूची को फिर से प्रकाशित करें।',
  'termPicker.deprecated':
    '“{value}” इस सूची में अप्रचलित है। यह तब तक इस आइटम पर बना रहता है जब तक आप एक वर्तमान प्रविष्टि नहीं चुनते।',
};
