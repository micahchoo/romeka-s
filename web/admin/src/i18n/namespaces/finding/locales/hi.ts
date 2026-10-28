/**
 * The Hindi jump-to box and saved list messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `FindingMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FindingMessages } from '../index';

export const hi: FindingMessages = {
  'jump.loading': '“{text}” की खोज की जा रही है…',
  'jump.none': 'आपको दिखने वाला कोई भी रिकॉर्ड “{text}” से मेल नहीं खाता।',
  'jump.many': '{count} रिकॉर्ड “{text}” से मेल खाते हैं',
  'jump.firstTen':
    'पहले {count} सूचीबद्ध हैं। उन्हें सीमित करने के लिए शीर्षक का अधिक हिस्सा टाइप करें।',
  'jump.failed': 'खोज पूरी नहीं हुई: {message}',
  'jump.close': 'बंद करें',
  'jump.kind.items': 'आइटम',
  'jump.kind.item_sets': 'आइटम सेट',
  'jump.kind.media': 'मीडिया',
  'jump.untitled': '[बिना नाम]',
  'savedLists.save': 'इस सूची को सहेजें',
  'savedLists.name': 'नाम',
  'savedLists.where':
    'ये फ़िल्टर और ये क्रम सहेजता है। नाम मेनू में आइटम्स के नीचे, किसी भी कंप्यूटर पर दिखाई देता है, जिस पर आप साइन इन करते हैं।',
  'savedLists.submit': 'सहेजें',
  'savedLists.saving': 'सहेजा जा रहा है…',
  'savedLists.cancel': 'रद्द करें',
  'savedLists.current': 'सहेजी गई सूची: {name}',
  'savedLists.rename': 'सहेजी गई सूची का नाम बदलें',
  'savedLists.delete': 'सहेजी गई सूची को हटाएँ',
  'savedLists.deleted': 'सहेजी गई सूची {name} को हटा दिया गया।',
  'savedLists.nameEmpty': 'सूची को एक नाम दें।',
  'savedLists.nameTaken': 'आपके पास पहले से ही {name} नाम की एक सूची है।',
  'savedLists.tooMany':
    'आपके पास {count} सहेजी गई सूचियाँ हैं। यह वह अधिकतम संख्या है जो एक व्यक्ति रख सकता है। पहले एक को हटा दें।',
  'savedLists.gone': 'यह सहेजी गई सूची किसी अन्य विंडो में हटा दी गई थी।',
  'savedLists.changed': 'सहेजे जाने के बाद से बदला गया',
  'savedLists.update': 'इन फ़िल्टरों के साथ {name} को अपडेट करें',
  'savedLists.updated': 'इन फ़िल्टरों के साथ {name} को अपडेट किया गया।',
  'savedLists.saveNew': 'एक नई सूची के रूप में सहेजें',
  'savedLists.revert': 'सहेजे गए फ़िल्टरों पर वापस जाएँ',
};
