/**
 * The Marathi Term lists messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `TermListMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { TermListMessages } from '../messages';

export const mr: TermListMessages = {
  'termLists.resource': 'शब्दसूची',
  'termLists.invalidId': '"{param}" ही शब्दसूची आयडी नाही.',
  'termLists.intro':
    'शब्दसूची एका क्षेत्राला निश्चित नोंदींच्या संचातून निवडीमध्ये रूपांतरित करते, जेणेकरून तीच जागा किंवा विषय नेहमी एकाच पद्धतीने लिहिला जातो.',
  'termLists.add': 'शब्दसूची जोडा',
  'termLists.legend': 'शब्दसूची',
  'termLists.kindLegend': 'या सूचीमध्ये काय आहे',
  'termLists.kindTerms': 'शब्दांची सूची',
  'termLists.kindItems': 'घटक संचातील घटक',
  'termLists.kindUris': 'लेबल्ससह यूआरआय लिंक्स',
  'termLists.kindEmpty': 'अद्याप काहीही नाही',
  'termLists.termsLabel': 'प्रति ओळ एक शब्द',
  'termLists.urisLabel': 'प्रति ओळ लिंक्स: यूआरआय, एक स्पेस, नंतर लेबल',
  'termLists.itemSetLabel': 'घटक संच',
  'termLists.selectItemSet': 'घटक संच निवडा',
  'termLists.langLabel': 'शब्दांची भाषा',
  'termLists.langHint':
    'पर्यायी. या सूचीमधून निवडलेला प्रत्येक शब्द या भाषा टॅगला वाहून नेतो, उदाहरणार्थ en.',
  'termLists.errorNoEntries': 'किमान एक नोंद जोडा.',
  'termLists.errorNoItemSet': 'घटक संच निवडा.',
  'termLists.columnKind': 'धारण',
  'termLists.columnEntries': 'नोंदी',
  'termLists.columnUsage': 'याचा वापर करणारे घटक',
  'termLists.editLabel': 'ही शब्दसूची संपादित करा',
  'termLists.deleteLabel': 'ही शब्दसूची हटवा',
  'termLists.deleteNote':
    'सूची हटवल्याने त्यातून निवडलेली मूल्ये काढून टाकली जातात. ती त्यांच्या घटकांवर साध्या मजकुराच्या स्वरूपात राहतात.',
  'termPicker.loading': 'सूची लोड होत आहे...',
  'termPicker.noMatch': '"{query}" शी जुळणारी कोणतीही नोंद नाही.',
  'termPicker.notInList': '"{value}" या सूचीमध्ये नाही. ते बदलण्यासाठी एखादी नोंद निवडा.',
  'termPicker.hint': '{list} सूची फिल्टर करण्यासाठी टाईप करा, नंतर एखादी नोंद निवडा.',
  'termPicker.typeLabel': 'शब्दसूची: {list}',
  'tags.title': 'टॅग्स',
  'tags.intro':
    'वर्णन करताना लोकांनी जोडलेले मोफत शब्द. जेव्हा काही जण वारंवार येतात, तेव्हा त्यांना एक सूची बनवा.',
  'tags.empty':
    'अद्याप कोणतेही टॅग्स नाहीत. ते आयटम, आयटम सेट किंवा मीडियाच्या टॅग्स क्षेत्रात जोडा.',
  'tags.legend': 'प्रसिद्ध करण्यासाठी टॅग्स',
  'tags.listName': 'सूचीचे नाव',
  'tags.listHint': 'शब्दांची नवीन सूची, किंवा शब्दांची सूची ज्यामध्ये तुम्ही टॅग्स जोडणार आहात.',
  'tags.promote': 'सूचीमध्ये प्रसिद्ध करा',
  'tags.promoteTitle': '“{list}” सूचीमध्ये प्रसिद्ध करा',
  'tags.newList': 'यामुळे एक नवीन सूची तयार होते.',
  'tags.fields': 'ज्या आयटम प्रकारांचा सब्जेक्ट फील्ड सूची प्रदान करेल',
  'tags.fieldsSkipped':
    'ज्या आयटम प्रकारांमध्ये तुम्ही बदल करू शकत नाही, ते जसे आहेत तसे ठेवले जातात',
  'tags.skipped': 'तुम्ही संपादित करू शकत नसलेले संसाधने, जे त्यांचे टॅग ठेवतात',
  'tags.promoted': '“{list}” सूचीमध्ये प्रसिद्ध केले. संसाधने बदलली: {changed}.',
  'rewrite.changes': 'बदलणारी संसाधने',
  'rewrite.skipped': 'ज्या संसाधनांमध्ये तुम्ही बदल करू शकत नाही, ज्यांची स्पेलिंग कायम राहते',
  'rewrite.none': 'काहीही नाही',
  'merge.title': 'समान स्पेलिंग',
  'merge.intro':
    'या एकाच मूल्यासारख्या दिसतात. त्यांना एकत्र करा जेणेकरून एका शोधामुळे त्या सर्वांचा शोध लागतो.',
  'merge.keep': '“{value}” ठेवा',
  'merge.usedBy': '{count} द्वारे वापरले जाणारे',
  'merge.into': '“{value}” मध्ये विलीन करा',
  'merge.confirm': 'विलीन करा',
  'merge.different': 'ते भिन्न आहेत',
  'merge.merged': '“{value}” मध्ये विलीन केले. संसाधने बदलली: {changed}.',
  'merge.remembered': 'नोंद घेतली. या स्पेलिंग्ज पुन्हा विलीन करण्यासाठी ऑफर केल्या जाणार नाहीत.',
};
