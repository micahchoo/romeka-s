/**
 * The Hindi import run screen messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `ImportRunMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ImportRunMessages } from '../index';

export const hi: ImportRunMessages = {
  'run.count.items': '{count} वस्तुएं',
  'run.count.itemsOne': '1 वस्तु',
  'run.count.files': '{count} फाइलें',
  'run.count.filesOne': '1 फाइल',
  'run.count.itemSets': '{count} आइटम सेट',
  'run.count.itemSetsOne': '1 आइटम सेट',
  'run.count.itemTypes': '{count} आइटम प्रकार',
  'run.count.itemTypesOne': '1 आइटम प्रकार',
  'run.count.fields': '{count} फ़ील्ड',
  'run.count.fieldsOne': '1 फ़ील्ड',
  'run.count.problems': '{count} समस्याएँ',
  'run.count.problemsOne': '1 समस्या',
  'run.kind.item': 'आइटम {id}',
  'run.kind.media': 'फ़ाइल {id}',
  'run.kind.itemSet': 'आइटम सेट {id}',
  'run.kind.resourceTemplate': 'आइटम प्रकार {id}',
  'run.kind.property': 'फ़ील्ड {id}',
  'run.empty.title': 'अभी कुछ भी जोड़ने के लिए नहीं है',
  'run.empty.note':
    'जब आपकी वस्तुएं तैयार हो जाएं, तो आप उन्हें यहाँ जोड़ें। आप बाद में पूरे आयात को पूर्ववत कर सकते हैं।',
  'run.empty.link': 'अपनी फ़ाइलें जोड़ें',
  'run.ready.counting': 'ओमेका में क्या जाता है, इसकी गणना...',
  'run.ready.saving': 'अपने अंतिम परिवर्तन को सहेजना। फिर गणना फिर से की जाती है।',
  'run.ready.counted': '{time} पर गणना की गई।',
  'run.ready.recount': 'फिर से गणना करें',
  'run.ready.must': '{what} जोड़ने से पहले ठीक किया जाना चाहिए।',
  'run.ready.look': 'उन पर गौर करें',
  'run.ready.nothing': 'ओमेका में कुछ भी नहीं जाता: हर वस्तु को बाहर रखा जाता है।',
  'run.ready.later': 'आप बाद में पूरे आयात को पूर्ववत कर सकते हैं।',
  'run.ready.add': 'ओमेका में {what} जोड़ें',
  'run.ready.delete': 'ओमेका से उन्हें हटा दें',
  'run.ready.starting': 'शुरू हो रहा है...',
  'run.confirm.note':
    'यह ओमेका से {count} को हटाता है: वस्तुएं, फाइलें और वस्तु सेट, एक साथ गिने गए। इसे पूर्ववत नहीं किया जा सकता।',
  'run.confirm.label': '{count} की पुष्टि करने के लिए टाइप करें',
  'run.no.changed':
    'आपके द्वारा गिने जाने के बाद आपके द्वारा फाइलें या आपके उत्तर बदल गए। अब गणना नए सिरे से की गई है: उन पर गौर करें, फिर जोड़ें।',
  'run.no.blocked': 'यह आयात शुरू नहीं हो सकता: {what} को पहले ठीक किया जाना चाहिए।',
  'run.no.confirm':
    'एक विलोपन के लिए इसके गणना को टाइप करने की आवश्यकता है: {count} वस्तुएं, फाइलें और वस्तु सेट, एक साथ गिने गए। इसे नीचे टाइप करें।',
  'run.no.busy':
    'अभी एक और आयात जोड़ा या हटाया जा रहा है। एक समय में केवल एक ही काम करता है। यह समाप्त होने पर फिर से प्रयास करें।',
  'run.no.busyOpen': 'उस आयात को खोलें',
  'run.no.busySelf': 'यह आयात अभी भी अपनी फाइलों को देख रहा है। कुछ देर में फिर से प्रयास करें।',
  'run.no.nothing': 'जोड़ने के लिए कुछ नहीं है: प्रत्येक वस्तु को बाहर रखा गया है।',
  'run.no.nothingLink': 'देखें कि हमने क्या पाया',
  'run.no.cannotRun':
    'इस आयात को अभी नहीं जोड़ा जा सकता। यह है: {status}। पृष्ठ अब दिखाता है कि यह कहाँ खड़ा है।',
  'run.no.cannotUndo':
    'इस आयात को अभी पूर्ववत नहीं किया जा सकता। यह है: {status}। पृष्ठ अब दिखाता है कि यह कहाँ खड़ा है।',
  'run.no.notRunning': 'यह आयात अब काम नहीं कर रहा है। पृष्ठ अब दिखाता है कि यह कहाँ खड़ा है।',
  'run.no.again': 'फिर से प्रयास करें',
  'run.work.adding': 'ओमेका में लिख रहे हैं...',
  'run.work.undoing': 'पूर्ववत कर रहे हैं...',
  'run.work.done': '{total} में से {done} पूर्ण।',
  'run.work.unit': 'प्रत्येक आइटम, प्रत्येक फ़ाइल और प्रत्येक आइटम सेट को एक गिना जाता है।',
  'run.work.minutes': 'इस गति पर {count} मिनट शेष हैं।',
  'run.work.minute': 'इस गति पर 1 मिनट शेष है।',
  'run.work.seconds': 'इस गति पर एक मिनट से भी कम समय बचा है।',
  'run.work.noTime': 'बचा हुआ समय तब दिखाता है जब पहला पूरा हो जाता है।',
  'run.work.leave':
    'आप इस पेज को छोड़ सकते हैं। आयात जारी रहता है, और यह पेज दिखाता है कि आप वापस आने पर कहाँ खड़े हैं।',
  'run.work.leaveUndo':
    'आप इस पेज को छोड़ सकते हैं। अनडू (undo) जारी रहता है, और यह पेज दिखाता है कि आप वापस आने पर कहाँ खड़े हैं।',
  'run.work.stop': 'हाथ में मौजूद एक के बाद रुकें',
  'run.work.stopNote':
    'ओमेका में जो अभी है, वह वहीं रहता है। फिर आप बाकी जोड़ सकते हैं, या इसे अनडू (undo) कर सकते हैं।',
  'run.work.stopUndo': 'हाथ में मौजूद एक के बाद अनडू (undo) रोकें',
  'run.work.stopUndoNote':
    'जो अनडू (undo) किया गया है, वह अनडू ही रहता है। फिर आप बाकी को अनडू कर सकते हैं।',
  'run.work.stopping': 'हाथ में मौजूद एक के बाद रुकना...',
  'run.restarted': 'पुनरारंभ द्वारा रोक दिया गया। यह अपने आप जारी रहता है।',
  'run.work.log': 'जॉब्स पेज पर लॉग देखें',
  'run.end.done': 'आयात पूरा हो गया है।',
  'run.end.stopped': 'आयात अंत से पहले रुक गया।',
  'run.end.asked': 'यह इसलिए रुका क्योंकि किसी ने इसे रोकने के लिए कहा।',
  'run.end.died': 'कार्य बिना पूरा किए रुक गया। इसे {time} पर रोका गया पाया गया।',
  'run.end.failed': '{count} विफल:',
  'run.end.failedOne': '1 विफल:',
  'run.end.what': 'क्या',
  'run.end.why': 'क्यों',
  'run.end.failures': 'विफल',
  'run.end.retry': 'फिर से प्रयास करें: {count} अभी पूरा नहीं हुआ',
  'run.end.retryNote': 'जो पहले ही किया जा चुका है, उसे छोड़ दिया जाता है।',
  'run.end.left': 'जो छूट गया या विफल रहा, उसकी सूची (.csv) डाउनलोड करें',
  'run.end.leftLoading': 'डाउनलोड किया जा रहा है...',
  'run.undo.open': 'इस आयात को पूर्ववत करें...',
  'run.undo.reading': 'यह पढ़ना कि पूर्ववत करने से क्या होगा...',
  'run.undo.removes': 'पूर्ववत करने से {what} हट जाता है जो इस आयात ने जोड़ा था।',
  'run.undo.restores': 'यह {what} को वापस वैसा ही कर देता है जैसा वे इस आयात से पहले थे।',
  'run.undo.empty': 'पूर्ववत करने के लिए कुछ भी नहीं है या वापस रखने के लिए कुछ भी नहीं है।',
  'run.undo.gone': '{count} ओमेका में अब नहीं हैं। पूर्ववत करना उन्हें छोड़ देता है।',
  'run.undo.goneOne': '1 ओमेका में अब नहीं है। पूर्ववत करना इसे छोड़ देता है।',
  'run.undo.kept':
    '{count} रखे गए हैं: चूंकि आयात के बाद, ओमेका में कुछ बदल गया या उनका उपयोग किया। प्रत्येक पंक्ति बताती है कि क्या। पूर्ववत करना उन्हें रखता है, जब तक कि आप "इसे वैसे भी पूर्ववत करें" न चुनें।',
  'run.undo.keptOne':
    '1 रखा गया है: चूंकि आयात के बाद, ओमेका में कुछ बदल गया या इसका उपयोग किया। इसकी पंक्ति बताती है कि क्या। पूर्ववत करना इसे रखता है, जब तक कि आप "इसे वैसे भी पूर्ववत करें" न चुनें।',
  'run.undo.anyway': 'इसे वैसे भी पूर्ववत करें',
  'run.undo.all': '{count} को वैसे भी पूर्ववत करें',
  'run.undo.list': 'रखा गया',
  'run.undo.loss':
    '{count} को वैसे भी पूर्ववत करना ओम्का में आयात के बाद उनके साथ किए गए कार्यों को भी पूर्ववत करता है।',
  'run.undo.lossOne':
    '1 को वैसे भी पूर्ववत करना ओम्का में आयात के बाद इसके साथ किए गए कार्यों को भी पूर्ववत करता है।',
  'run.undo.lossTypes':
    'एक आइटम प्रकार या एक फ़ील्ड जिसे पूर्ववत किया गया है, वह आयात के बाहर की वस्तुओं पर अपने मान भी खो देता है।',
  'run.undo.go': 'आयात को पूर्ववत करें',
  'run.undo.cancel': 'रद्द करें',
  'run.undo.starting': 'पूर्ववत करना शुरू करें...',
  'run.undone.done': 'पूर्ववत किया गया।',
  'run.undone.partly': 'आंशिक रूप से पूर्ववत।',
  'run.undone.failed': 'पूर्ववत प्रक्रिया अंत से पहले रुक गई।',
  'run.undone.removed': '{what} को हटा दिया गया।',
  'run.undone.restored': '{what} को वापस रखा गया।',
  'run.undone.anyway':
    '{count} को पूर्ववत कर दिया गया, हालांकि ओमेका में कुछ बदल गया या आयात के बाद उनका उपयोग किया गया।',
  'run.undone.anywayOne':
    '1 को पूर्ववत कर दिया गया, हालांकि ओमेका में कुछ बदल गया या आयात के बाद इसका उपयोग किया गया।',
  'run.undone.gone': '{count} ओमेका में अब मौजूद नहीं थे।',
  'run.undone.goneOne': '1 ओमेका में अब मौजूद नहीं था।',
  'run.undone.asked': '{total} में से {done} के बाद यह रुक गया, क्योंकि किसी ने इसे रोकने को कहा।',
  'run.undone.error': '{total} में से {done} के बाद यह रुक गया: {reason}',
  'run.undone.kept':
    '{count} को रखा गया: आयात के बाद, ओमेका में कुछ बदल गया या उनका उपयोग किया गया।',
  'run.undone.keptOne': '1 को रखा गया: आयात के बाद, ओमेका में कुछ बदल गया या इसका उपयोग किया गया।',
  'run.undone.rest': 'बाकी को पूर्ववत करें',
  'advanced.runs.counts': 'रन क्या लिखता है, रिकॉर्ड के प्रकार और कार्रवाई के अनुसार।',
  'advanced.runs.kind': 'रिकॉर्ड का प्रकार',
  'advanced.runs.does': 'क्या',
  'advanced.runs.count': 'संख्या',
  'advanced.runs.leave': 'बाहर रखा गया',
  'advanced.runs.go': 'आयात चलाएँ',
};
