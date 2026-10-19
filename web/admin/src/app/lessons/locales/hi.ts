/**
 * The Hindi lessons messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `LessonsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { LessonsMessages } from '../messages';

export const hi: LessonsMessages = {
  'lesson.label': 'पाठ: {title}',
  'lesson.step': 'चरण {number}/{count}: {rung}',
  'lesson.gotIt': 'समझ गया',
  'lesson.recall': 'पाठ दिखाएँ: {title}',
  'lesson.learnMore': '{standard} के बारे में पढ़ें',
  'rung.describe': 'वर्णन करें',
  'rung.kinds': 'चीजों के प्रकार',
  'rung.fixedValues': 'निश्चित मान',
  'rung.linkedData': 'लिंक्ड डेटा',
  'rung.makingStandards': 'मानक बनाना',
  'standard.dublinCore': 'डब्लिन कोर',
  'standard.rdfClasses': 'RDF स्कीमा में क्लासेज',
  'standard.linkedData': 'लिंक्ड डेटा',
  'standard.localVocabulary': 'Omeka S में शब्दावलियाँ',
  'term.title': 'मानक शब्द',
  'term.body':
    'प्रत्येक फ़ील्ड के नीचे कोड, जैसे dcterms:creator, डब्लिन कोर से उसका मानक शब्द है। मान उस शब्द के तहत सहेजे जाते हैं, इसलिए डब्लिन कोर पढ़ने वाला दूसरा संग्रह बिना पूछे यह जानता है कि प्रत्येक मान क्या है।',
  'rename.title': 'नाम और शब्द',
  'rename.body':
    'आपने फ़ॉर्म पर लोगों को दिखने वाला नाम बदल दिया है। मान अभी भी उसी मानक शब्द के तहत सहेजा जाता है, इसलिए खोज, निर्यात और अन्य संग्रह इसे पहले की तरह पढ़ते हैं।',
  'fieldsPage.title': 'साझा फ़ील्ड',
  'fieldsPage.body':
    'यहाँ हर फ़ील्ड साझा शब्दावली से एक शब्द है, अक्सर डब्लिन कोर। जब दो संग्रह दोनों एक निर्माता को dcterms:creator के रूप में सहेजते हैं, तो उनके रिकॉर्ड खोजे जा सकते हैं और एक साथ मिलाए जा सकते हैं।',
  'required.title': 'आवश्यक फ़ील्ड',
  'required.body':
    'यह आइटम प्रकार फ़ील्ड को आवश्यक के रूप में चिह्नित करता है, इसलिए इस प्रकार के प्रत्येक आइटम का यहाँ एक मान होता है। एक संग्रह जहाँ प्रत्येक रिकॉर्ड में यह है, उसे बिना किसी अंतराल के क्रमबद्ध और खोजा जा सकता है।',
  'typeClass.title': 'मानक वर्ग',
  'typeClass.body':
    'नाम लोगों के लिए है। मानक वर्ग, जैसे bibo:Letter, सॉफ्टवेयर को बताता है कि प्रत्येक आइटम क्या है, इसलिए इस वर्ग के लिए खोज, यहाँ या किसी अन्य संग्रह में, इस प्रकार के प्रत्येक आइटम को ढूंढ लेती है।',
  'typePick.title': 'आइटम प्रकार',
  'typePick.body':
    'एक प्रकार एक तरह की चीज़ के लिए फ़ील्ड लेआउट करता है और मानक वर्ग के साथ आइटम सहेजता है। एक प्रकार के आइटम एक ही फ़ील्ड साझा करते हैं, इसलिए उनकी तुलना करना और एक साथ ढूंढना आसान होता है।',
  'standards.title': 'लिंक्ड डेटा',
  'standards.body':
    'ये वे शब्दावलियाँ हैं जो हर फ़ील्ड और वर्ग के पीछे होती हैं। प्रत्येक शब्द का एक वेब पता होता है, जैसे http://purl.org/dc/terms/title, इसलिए यहाँ सहेजा गया मान कहीं भी पढ़े जाने पर समान अर्थ रखता है।',
  'localField.title': 'आपकी अपनी शब्दावली',
  'localField.body':
    'इसका नाम local: से शुरू होता है क्योंकि किसी साझा मानक में यह फ़ील्ड नहीं थी, इसलिए यह इस संग्रह की अपनी शब्दावली में एक शब्द है। इसका पता इस साइट पर है, इसलिए अन्य संग्रह मान को पढ़ सकते हैं और यह देख सकते हैं कि इसका क्या अर्थ है।',
  'switch.label': 'पाठ दिखाएँ',
  'switch.hint': 'मानक के पीछे के मानक को समझाने वाले संक्षिप्त नोट, पहली बार जब आप इसे लेते हैं।',
};
