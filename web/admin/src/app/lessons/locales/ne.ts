/**
 * The Nepali lessons messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `LessonsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { LessonsMessages } from '../messages';

export const ne: LessonsMessages = {
  'lesson.label': 'पाठ: {title}',
  'lesson.step': 'चरण {number}/{count}: {rung}',
  'lesson.gotIt': 'बुझियो',
  'lesson.recall': 'पाठ देखाउनुहोस्: {title}',
  'lesson.learnMore': '{standard} को बारेमा पढ्नुहोस्',
  'rung.describe': 'वर्णन गर्नुहोस्',
  'rung.kinds': 'वस्तुका प्रकारहरू',
  'rung.fixedValues': 'निश्चित मानहरू',
  'rung.linkedData': 'लिङ्क गरिएको डेटा',
  'rung.makingStandards': 'मानकहरू बनाउने',
  'standard.dublinCore': 'डब्लिन कोर',
  'standard.rdfClasses': 'RDF स्किमामा क्लासहरू',
  'standard.linkedData': 'लिङ्क गरिएको डेटा',
  'standard.localVocabulary': 'Omeka S मा शब्दावलीहरू',
  'term.title': 'मानक शब्दहरू',
  'term.body':
    'प्रत्येक फिल्ड मुनिको कोड, जस्तै dcterms:creator, डब्लिन कोरबाट यसको मानक शब्द हो। मानहरू त्यस शब्द मुनि सुरक्षित गरिन्छ, त्यसैले डब्लिन कोर पढ्ने अर्को आर्काइभले तपाईँलाई नभनीकनै प्रत्येक मान के हो भन्ने जान्दछ।',
  'rename.title': 'नाम र शब्दहरू',
  'rename.body':
    'तपाईँले फर्ममा मानिसहरूले देख्ने नाम परिवर्तन गर्नुभयो। मान अझै पनि उही मानक शब्द मुनि सुरक्षित छ, त्यसैले खोज, निर्यात र अन्य आर्काइभहरूले यसलाई पहिले जस्तै पढ्छन्।',
  'fieldsPage.title': 'साझा फिल्डहरू',
  'fieldsPage.body':
    'यहाँका प्रत्येक फिल्ड साझा शब्दावलीको एउटा शब्द हो, जसमा प्राय डब्लिन कोर हुन्छ। जब दुईवटा आर्काइभले दुवैले dcterms:creator को रूपमा निर्माता सुरक्षित गर्छन्, तिनीहरूका रेकर्डहरूलाई सँगै खोज्न र मर्ज गर्न सकिन्छ।',
  'required.title': 'आवश्यक फिल्डहरू',
  'required.body':
    'यो वस्तु प्रकारले फिल्डलाई आवश्यकको रूपमा चिन्हित गर्दछ, त्यसैले यस प्रकारको प्रत्येक वस्तुमा यहाँ एउटा मान हुन्छ। एउटा सङ्ग्रह जहाँ प्रत्येक रेकर्डमा यो छ, त्यसलाई कुनै खाली ठाउँ बिना क्रमबद्ध र खोज्न सकिन्छ।',
  'typeClass.title': 'मानक वर्गहरू',
  'typeClass.body':
    'नाम मानिसहरूका लागि हो। मानक वर्ग, जस्तै bibo:Letter, ले सफ्टवेयरलाई प्रत्येक वस्तु कस्तो प्रकारको कुरा हो भन्ने बताउँछ, ताकि यस वर्गको खोज, यहाँ वा अर्को आर्काइभमा, यस प्रकारको प्रत्येक वस्तु फेला पार्छ।',
  'typePick.title': 'वस्तु प्रकारहरू',
  'typePick.body':
    'एउटा प्रकारले एक प्रकारको कुराका लागि फिल्डहरू राख्छ र मानक वर्गसँगै वस्तु सुरक्षित गर्छ। एउटै प्रकारका वस्तुहरूले एउटै फिल्डहरू साझा गर्छन्, त्यसैले तिनीहरूलाई तुलना गर्न र सँगै फेला पार्न सजिलो हुन्छ।',
  'standards.title': 'लिङ्क गरिएको डेटा',
  'standards.body':
    'यी प्रत्येक फिल्ड र वर्ग पछाडिका शब्दावलीहरू हुन्। प्रत्येक शब्दको एउटा वेब ठेगाना हुन्छ, जस्तै http://purl.org/dc/terms/title, त्यसैले यहाँ सुरक्षित गरिएको मान जहाँ पढिएको भए पनि एउटै अर्थ हुन्छ।',
  'localField.title': 'तपाईँको आफ्नै शब्दावली',
  'localField.body':
    'यसको नाम local: बाट सुरु हुन्छ किनभने यस फिल्डका लागि कुनै साझा मानक थिएन, त्यसैले यो यस आर्काइभको आफ्नै शब्दावलीमा एउटा शब्द हो। यसको ठेगाना यस साइटमा छ, त्यसैले अन्य आर्काइभहरूले अझै पनि मान पढ्न सक्छन् र यसको अर्थ के हो भनेर खोज्न सक्छन्।',
  'switch.label': 'पाठहरू देखाउनुहोस्',
  'switch.hint': 'कुनै कार्य पहिलो पटक गर्दा त्यसको मानक पछाडिको छोटो नोटहरू।',
};
