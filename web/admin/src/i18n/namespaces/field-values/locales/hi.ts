/**
 * The Hindi values board messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `FieldValuesMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FieldValuesMessages } from '../index';

export const hi: FieldValuesMessages = {
  'values.title': '{field} के मान',
  'values.chooseIntro':
    'उन टेक्स्ट्स को देखने के लिए एक फ़ील्ड चुनें जो आपके आइटम और मीडिया में वे जो रखते हैं, और एक टेक्स्ट को उस फ़ील्ड में ले जाएँ जिससे वह संबंधित है।',
  'values.inUse': 'आइटम प्रकारों द्वारा उपयोग किए जाने वाले फ़ील्ड',
  'values.noneInUse': 'किसी भी आइटम प्रकार में अभी तक कोई फ़ील्ड नहीं है। नीचे एक फ़ील्ड खोजें।',
  'values.usedByOne': '1 आइटम प्रकार में',
  'values.usedBy': '{count} आइटम प्रकारों में',
  'values.findOther': 'एक और फ़ील्ड खोजें',
  'values.findOtherHint': 'इसका वर्णन करें, उदाहरण के लिए “यह कहाँ बनाया गया था”',
  'values.matches': '“{text}” के लिए मिलान',
  'values.noFieldMatch': 'कोई फ़ील्ड “{text}” से मेल नहीं खाता।',
  'values.allFields': 'सभी फ़ील्ड',
  'values.intro':
    'इस फ़ील्ड में रखी गई हर टेक्स्ट, आइटम, आइटम सेट और मीडिया पर, सबसे अधिक उपयोग की जाने वाली पहले। किसी मान को दूसरे फ़ील्ड में खींचें, या "मूव टू" का उपयोग करें, और यह हर उस रिकॉर्ड पर चला जाता है जिसमें वह मान होता है।',
  'values.loading': 'मानों को पढ़ना…',
  'values.noField': 'किसी भी फ़ील्ड में संख्या {id} नहीं है।',
  'values.addColumn': 'फ़ील्ड जोड़ें',
  'values.chooseField': 'एक फ़ील्ड चुनें',
  'values.removeColumn': '{field} कॉलम हटाएँ',
  'values.countOne': '1 मान',
  'values.countMany': '{count} मान',
  'values.find': 'मान खोजें',
  'values.empty': 'किसी भी रिकॉर्ड में {field} में अभी तक कोई मान नहीं है।',
  'values.noMatch': 'किसी भी मान में “{text}” नहीं है।',
  'values.dropHere': 'मानों को {field} में ले जाने के लिए यहाँ छोड़ें।',
  'values.showMore': '{count} और अधिक दिखाएँ',
  'values.andMore': 'और {count} अधिक',
  'values.itemsOne': '1 आइटम',
  'values.itemsMany': '{count} आइटम',
  'values.mediaCount': '{count} मीडिया',
  'values.setsOne': '1 आइटम सेट',
  'values.setsMany': '{count} आइटम सेट',
  'values.choose': '“{value}” चुनें',
  'values.chosen':
    '{count} मान चुने गए। उनमें से एक को ड्रैग करें, या सभी को स्थानांतरित करने के लिए एक पर “Move to” का उपयोग करें।',
  'values.clearChosen': 'विकल्प साफ़ करें',
  'values.moveTo': '…स्थानांतरित करें',
  'values.moveLabel': '“{value}” को स्थानांतरित करें',
  'values.show': 'रिकॉर्ड दिखाएं',
  'values.hide': 'रिकॉर्ड छिपाएं',
  'values.reading': 'रिकॉर्ड्स पढ़ रहे हैं…',
  'values.openItems': 'इन आइटमों को आइटम्स में खोलें',
  'values.readOnly': 'आप इन रिकॉर्ड्स को बदल नहीं सकते।',
  'values.partEditable': 'आप {total} में से {editable} बदल सकते हैं।',
  'values.checkTitle': 'स्थानांतरण की जांच करें',
  'values.checking': 'यह जांचना कि क्या बदलाव होगा…',
  'values.previewOne': '1 रिकॉर्ड पर “{value}” को {from} से {to} में ले जाएँ।',
  'values.previewMany': '{count} रिकॉर्ड पर “{value}” को {from} से {to} में ले जाएँ।',
  'values.merged': 'इनमें से {count} में यह पहले से {to} में है; उनसे यह केवल {from} से हटेगा।',
  'values.skipped': '{count} रिकॉर्ड, जिन्हें आप बदल नहीं सकते, इसे {from} में ही रखेंगे।',
  'values.nothing':
    'स्थानांतरित करने के लिए कुछ नहीं: कोई भी रिकॉर्ड जिसे आप बदल सकते हैं, उसमें “{value}” नहीं है।',
  'values.notInForm':
    '{to}, {type} के फ़ील्ड में नहीं है: इनमें से {count} रिकॉर्ड में ऐसा मान होगा जिसे उसका फ़ॉर्म नहीं दिखाता।',
  'values.addToType': '{type} में {to} जोड़ें',
  'values.move': 'स्थानांतरण',
  'values.moving': 'स्थानांतरण…',
  'values.doneOne': '“{value}” को {count} रिकॉर्ड पर {to} में ले जाया गया।',
  'values.doneMany': '{values} मान {count} रिकॉर्ड पर {to} में ले जाए गए।',
  'values.undo': 'पूर्ववत (Undo)',
  'values.undoing': 'इसे वापस रख रहे हैं…',
  'values.undone': '“{value}” वापस {from} में रख दिया गया।',
  'values.history': 'इस विज़िट पर स्थानांतरण',
  'values.historyRow': '“{value}”: {from} → {to}, {count} रिकॉर्ड्स',
  'values.undoLast': 'अंतिम स्थानांतरण को पूर्ववत करें',
  'values.undoNote':
    'पूर्ववत (Undo) इस विज़िट पर किए गए अंतिम स्थानांतरण को वापस रखता है। प्रत्येक आइटम का इतिहास प्रत्येक स्थानांतरण को सूचीबद्ध करता है।',
};
