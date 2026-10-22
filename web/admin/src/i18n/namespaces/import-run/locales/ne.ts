/**
 * The Nepali import run screen messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `ImportRunMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ImportRunMessages } from '../index';

export const ne: ImportRunMessages = {
  'run.count.items': '{count} वस्तुहरू',
  'run.count.itemsOne': '1 वस्तु',
  'run.count.files': '{count} फाइलहरू',
  'run.count.filesOne': '1 फाइल',
  'run.count.itemSets': '{count} वस्तु समूहहरू',
  'run.count.itemSetsOne': '1 वस्तु समूह',
  'run.count.itemTypes': '{count} वस्तु प्रकारहरू',
  'run.count.itemTypesOne': '1 वस्तु प्रकार',
  'run.count.fields': '{count} क्षेत्रहरू',
  'run.count.fieldsOne': '1 क्षेत्र',
  'run.count.problems': '{count} समस्याहरू',
  'run.count.problemsOne': '1 समस्या',
  'run.kind.item': 'वस्तु {id}',
  'run.kind.media': 'फाइल {id}',
  'run.kind.itemSet': 'वस्तु समूह {id}',
  'run.kind.resourceTemplate': 'वस्तु प्रकार {id}',
  'run.kind.property': 'क्षेत्र {id}',
  'run.empty.title': 'अहिलेसम्म केही थप्नुपर्ने छैन',
  'run.empty.note':
    'जब तपाईँका वस्तुहरू तयार हुन्छन्, यहाँ थप्नुहोस्। तपाईँले पछि सम्पूर्ण आयातलाई पूर्ववत गर्न सक्नुहुन्छ।',
  'run.empty.link': 'आफ्ना फाइलहरू थप्नुहोस्',
  'run.ready.counting': 'Omeka मा के गणना गरिँदैछ...',
  'run.ready.saving': 'अन्तिम परिवर्तन सुरक्षित गर्दै। त्यसपछि गणनाहरू पुनः गरिन्छ।',
  'run.ready.counted': '{time} मा गणना गरियो।',
  'run.ready.recount': 'पुनः गणना गर्नुहोस्',
  'run.ready.must': '{what} थप्नु अघि समाधान गर्नुपर्छ।',
  'run.ready.look': 'तिनीहरूलाई हेर्नुहोस्',
  'run.ready.nothing': 'Omeka मा केही पनि जाँदैन: प्रत्येक वस्तुलाई बाहिर राखिन्छ।',
  'run.ready.later': 'तपाईँले पछि सम्पूर्ण आयातलाई पूर्ववत गर्न सक्नुहुन्छ।',
  'run.ready.add': 'Omeka मा {what} थप्नुहोस्',
  'run.ready.delete': 'Omeka बाट तिनीहरूलाई मेटाउनुहोस्',
  'run.ready.starting': 'सुरु हुँदैछ...',
  'run.confirm.note':
    'यसले Omeka बाट {count} मेटाउँछ: वस्तुहरू, फाइलहरू र वस्तु सेटहरू, सबैलाई सँगै गणना गरिएको। यसलाई पूर्ववत गर्न सकिँदैन।',
  'run.confirm.label': '{count} पुष्टि गर्न टाइप गर्नुहोस्',
  'run.no.changed':
    'तपाईँका फाइलहरू वा तपाईँका उत्तरहरू गणना गरिसकेपछि परिवर्तन भए। अब गणनाहरू नयाँ छन्: तिनीहरूलाई हेर्नुहोस्, त्यसपछि थप्नुहोस्।',
  'run.no.blocked': 'यो आयात सुरु हुन सक्दैन: {what} लाई पहिले सुधार्नुपर्छ।',
  'run.no.confirm':
    'एउटा मेटाउनका लागि यसको गणना टाइप गर्न आवश्यक छ: {count} वस्तुहरू, फाइलहरू र वस्तु सेटहरू, सँगै गणना गरिएको। तल टाइप गर्नुहोस्।',
  'run.no.busy':
    'अहिले अर्को आयात थपिँदैछ वा हटाइँदैछ। एक पटकमा एउटा मात्र काम गर्न सक्छ। यो समाप्त भएपछि फेरि प्रयास गर्नुहोस्।',
  'run.no.busyOpen': 'त्यो आयात खोल्नुहोस्',
  'run.no.busySelf':
    'यो आयातले अझै पनि आफ्ना फाइलहरू हेरिरहेको छ। केही समयमा फेरि प्रयास गर्नुहोस्।',
  'run.no.nothing': 'थप्नका लागि केही छैन: प्रत्येक वस्तु बाहिर छ।',
  'run.no.nothingLink': 'हामीले के पायौँ हेर्नुहोस्',
  'run.no.cannotRun':
    'यो आयात अहिले थप्न सकिँदैन। यो: {status} छ। पृष्ठले अहिले यसको स्थिति देखाउँछ।',
  'run.no.cannotUndo': 'यो आयात अहिले हटाइँदैन। यो: {status} छ। पृष्ठले अहिले यसको स्थिति देखाउँछ।',
  'run.no.notRunning': 'यो आयातले अब काम गर्दैन। पृष्ठले अहिले यसको स्थिति देखाउँछ।',
  'run.no.again': 'फेरि प्रयास गर्नुहोस्',
  'run.work.adding': 'Omeka मा लेख्दै...',
  'run.work.undoing': 'पूर्ववत गर्दै...',
  'run.work.done': '{total} मध्ये {done} पुरा भयो।',
  'run.work.unit': 'प्रत्येक वस्तु, प्रत्येक फाइल र प्रत्येक वस्तु समूहले एउटा गणना गर्छ।',
  'run.work.minutes': 'यस गतिमा {count} मिनेट बाँकी छ।',
  'run.work.minute': 'यस गतिमा 1 मिनेट बाँकी छ।',
  'run.work.seconds': 'यस गतिमा एक मिनेटभन्दा कम बाँकी छ।',
  'run.work.noTime': 'बाँकी समयले पहिलो काम पुरा भएको देखाउँछ।',
  'run.work.leave':
    'तपाईँ यो पृष्ठ छोड्न सक्नुहुन्छ। आयात जारी रहन्छ, र यो पृष्ठले तपाईँ फर्कँदा यो कहाँ छ भनेर देखाउँछ।',
  'run.work.leaveUndo':
    'तपाईँ यो पृष्ठ छोड्न सक्नुहुन्छ। पूर्ववत प्रक्रिया जारी रहन्छ, र यो पृष्ठले तपाईँ फर्कँदा यो कहाँ छ भनेर देखाउँछ।',
  'run.work.stop': 'हातमा रहेको एकलाई रोक्नुहोस्',
  'run.work.stopNote':
    'Omeka मा अहिले जे छ, त्यही त्यहाँ रहन्छ। त्यसपछि तपाईँले बाँकी थप्न सक्नुहुन्छ, वा यसलाई पूर्ववत गर्न सक्नुहुन्छ।',
  'run.work.stopUndo': 'हातमा रहेको एकलाई पूर्ववत गरेपछि रोक्नुहोस्',
  'run.work.stopUndoNote':
    'जे पूर्ववत हुन्छ, त्यो पूर्ववत नै रहन्छ। त्यसपछि तपाईँले बाँकीलाई पूर्ववत गर्न सक्नुहुन्छ।',
  'run.work.stopping': 'हातमा रहेको एकलाई रोकिँदैछ...',
  'run.work.resumed': 'पुन: सुरु गर्दा रोकियो। यो आफैँ अघि बढ्छ।',
  'run.work.log': 'Jobs पृष्ठमा लग हेर्नुहोस्',
  'run.end.done': 'आयात पुरा भयो।',
  'run.end.stopped': 'आयात अन्त्य हुनुभन्दा अघि रोकियो।',
  'run.end.asked': 'कसैले यसलाई रोक्न भनेकाले यो रोकियो।',
  'run.end.died': 'काम पुरा नभई रोकियो। यो {time} मा रोकिएको फेला पर्यो।',
  'run.end.failed': '{count} असफल:',
  'run.end.failedOne': '1 असफल:',
  'run.end.what': 'के',
  'run.end.why': 'किन',
  'run.end.failures': 'असफल',
  'run.end.retry': 'फेरि प्रयास गर्नुहोस्: {count} अझै बाँकी छ',
  'run.end.retryNote': 'पहिले नै पुरा भइसकेका कुराहरूलाई छोडिन्छ।',
  'run.end.left': 'बाँकी रहेका वा असफल भएका कुराहरूको सूची (.csv) डाउनलोड गर्नुहोस्',
  'run.end.leftLoading': 'डाउनलोड हुँदैछ...',
  'run.undo.open': 'यो आयातलाई पूर्ववत गर्नुहोस्...',
  'run.undo.reading': 'पूर्ववत गर्दा के हुन्छ भनेर पढ्दै...',
  'run.undo.removes': 'पूर्ववत गर्दा यस आयातले थपेका {what} लाई हटाइन्छ।',
  'run.undo.restores':
    'यसले {what} लाई उनीहरू यस आयातले परिवर्तन गर्नुअघि जस्तै अवस्थामा फर्काउँछ।',
  'run.undo.empty': 'पूर्ववत गर्नका लागि केही पनि हटाउनु पर्दैन वा फिर्ता राख्नु पर्दैन।',
  'run.undo.gone': '{count} ओम्का (Omeka) मा छैनन्। पूर्ववत गर्दा ती छोडिन्छन्।',
  'run.undo.goneOne': '1 ओम्का (Omeka) मा छैन। पूर्ववत गर्दा यसलाई छोडिन्छ।',
  'run.undo.kept':
    '{count} राखिएका छन्: आयातदेखि, ओम्का (Omeka) मा केही परिवर्तन भएको छ वा ती प्रयोग भइरहेका छन्। प्रत्येक पङ्क्तिले के भन्छ। पूर्ववतले तिनीहरूलाई राख्छ, जबसम्म तपाईँ “जे भए पनि पूर्ववत गर्नुहोस्” रोज्नुहुन्न।',
  'run.undo.keptOne':
    '1 राखिएको छ: आयातदेखि, ओम्का (Omeka) मा केही परिवर्तन भएको छ वा यसको प्रयोग भइरहेको छ। यसको पङ्क्तिले के भन्छ। पूर्ववतले यसलाई राख्छ, जबसम्म तपाईँ “जे भए पनि पूर्ववत गर्नुहोस्” रोज्नुहुन्न।',
  'run.undo.anyway': 'जे भए पनि पूर्ववत गर्नुहोस्',
  'run.undo.all': 'जे भए पनि {count} पूर्ववत गर्नुहोस्',
  'run.undo.list': 'राखिएको',
  'run.undo.loss':
    'जे भए पनि {count} पूर्ववत गर्दा आयात पछिको ओम्का (Omeka) मा उनीहरूमाथि गरिएको काम पनि पूर्ववत हुन्छ।',
  'run.undo.lossOne':
    'जे भए पनि 1 पूर्ववत गर्दा आयात पछिको ओम्का (Omeka) मा यसमा गरिएको काम पनि पूर्ववत हुन्छ।',
  'run.undo.lossTypes':
    'कुनै वस्तुको प्रकार वा क्षेत्र जसलाई पूर्ववत गरिएको छ, त्यसले आयात बाहिरका वस्तुहरूमा रहेका यसका मानहरू पनि गुमाउँछ।',
  'run.undo.go': 'आयात पूर्ववत गर्नुहोस्',
  'run.undo.cancel': 'रद्द गर्नुहोस्',
  'run.undo.starting': 'पूर्ववत प्रक्रिया सुरु गर्दै...',
  'run.undone.done': 'पूर्ववत गरियो।',
  'run.undone.partly': 'आंशिक रूपमा पूर्ववत गरियो।',
  'run.undone.failed': 'पूर्ववत प्रक्रिया अन्त्य हुनुभन्दा अघि रोकियो।',
  'run.undone.removed': '{what} हटाइयो।',
  'run.undone.restored': '{what} पुनः राखियो।',
  'run.undone.anyway':
    '{count} लाई पूर्ववत गरियो, यद्यपि ओमेकाको केही परिवर्तनले आयात पछि तिनीहरूलाई परिवर्तन गर्‍यो वा प्रयोग गर्‍यो।',
  'run.undone.anywayOne':
    '1 लाई पूर्ववत गरियो, यद्यपि ओमेकाको केही परिवर्तनले आयात पछि यसलाई परिवर्तन गर्‍यो वा प्रयोग गर्‍यायो।',
  'run.undone.gone': '{count} ओमेकाबाट हटाइयो।',
  'run.undone.goneOne': '1 ओमेकाबाट हटाइयो।',
  'run.undone.asked': '{total} मध्ये {done} पछि यो रोकियो, किनकि कसैले यसलाई रोक्न भन्यो।',
  'run.undone.error': '{total} मध्ये {done} पछि {reason} का कारण यो रोकियो।',
  'run.undone.kept':
    '{count} राखियो: ओमेकाबाट आयात पछि, केही परिवर्तनले तिनीहरूलाई परिवर्तन गर्‍यो वा प्रयोग गर्‍यो।',
  'run.undone.keptOne':
    '1 राखियो: ओमेकाबाट आयात पछि, केही परिवर्तनले यसलाई परिवर्तन गर्‍यो वा प्रयोग गर्‍यायो।',
  'run.undone.rest': 'बाँकीलाई पूर्ववत गर्नुहोस्',
  'advanced.runs.counts': 'रनले के लेख्छ, रेकर्डको प्रकार र कार्य अनुसार।',
  'advanced.runs.kind': 'रेकर्डको प्रकार',
  'advanced.runs.does': 'के',
  'advanced.runs.count': 'गणना',
  'advanced.runs.leave': 'छुटेको',
  'advanced.runs.go': 'आयात चलाउनुहोस्',
};
