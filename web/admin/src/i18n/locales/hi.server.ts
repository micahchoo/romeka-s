/**
 * The Hindi server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const hiServer: ServerCatalogue = {
  'server.methodNotAllowed': 'विधि की अनुमति नहीं है',
  'server.notFound': 'नहीं मिला',
  'server.badRequest': 'खराब अनुरोध',
  'server.requestTimeout': 'अनुरोध का समय समाप्त',
  'server.bodyTooLarge': 'अनुरोध का शरीर बहुत बड़ा है',
  'server.internalError': 'आंतरिक सर्वर त्रुटि',
  'server.tooManyRequests': 'बहुत अधिक अनुरोध',
  'server.unknownResource': 'API "{resource}" संसाधन का समर्थन नहीं करता है।',
  'server.adapterNotImplemented': '{resource} एडेप्टर {operation} ऑपरेशन लागू नहीं करता है।',
  'server.endpointNotImplemented': '{endpoint} एंडपॉइंट अभी तक लागू नहीं किया गया है।',
  'server.permissionDenied':
    '{operation} {resource} संसाधन के लिए वर्तमान उपयोगकर्ता को अनुमति नहीं है।',
  'server.entityNotFound': '{class} मानदंड {criteria} के साथ नहीं मिला',
  'server.jsonSyntax': 'JSON: सिंटैक्स त्रुटि',
  'server.jsonNotContainer': 'JSON: सामग्री एक ऑब्जेक्ट या ऐरे होनी चाहिए।',
  'server.invalidContentType':
    'अमान्य Content-Type हेडर। "application/json" की अपेक्षा, प्राप्त हुआ "{received}"।',
  'server.bodyNotObject': 'अनुरोध बॉडी एक JSON ऑब्जेक्ट होनी चाहिए।',
  'server.siteItemsOneSelector': 'नाम या तो "query" होना चाहिए, या "items", "item_sets" या दोनों',
  'server.invalidCredentials': 'ईमेल या पासवर्ड अमान्य है',
  'server.inactiveAccount': 'उपयोगकर्ता सक्रिय नहीं है।',
  'server.unknownKey': 'अमान्य पासवर्ड निर्माण कुंजी।',
  'server.authenticationRequired': 'प्रमाणीकरण आवश्यक है।',
  'server.invalidPassword': 'पासवर्ड अमान्य है',
  'server.crossOriginRefused': 'सत्र-प्रमाणित लेखन कॉन्फ़िगर किए गए बेस URL से आना चाहिए।',
  'server.propertyNotRecognized': 'गुण "{term}" मान्यता प्राप्त नहीं है।',
  'server.resourceNotFound': 'id {id} के साथ संसाधन नहीं मिला।',
  'server.templateRequiresValue':
    '"{template}" संसाधन टेम्पलेट के लिए एक "{property}" मान आवश्यक है',
  'server.itemSetsNotArray': 'आइटम सेट एक ऐरे होना चाहिए',
  'server.mediaNotArray': 'मीडिया एक ऐरे होना चाहिए',
  'server.mediaNotThisItem': 'मीडिया इस आइटम से संबंधित होना चाहिए।',
  'server.primaryMediaNotThisItem': 'प्राथमिक मीडिया इस आइटम से संबंधित होना चाहिए।',
  'server.mediaIngesterRequired': 'मीडिया को एक इंजस्टर (ingester) सेट करना चाहिए।',
  'server.mediaIngesterInvalid': 'मीडिया को एक वैध इंजस्टर सेट करना चाहिए।',
  'server.mediaItemRequired': 'मीडिया को एक आइटम से संबंधित होना चाहिए।',
  'server.jobHeldByImport':
    'यह कार्य एक संग्रह आयात से संबंधित है जो चल रहा है या पूर्ववत हो रहा है। आयात समाप्त होने के बाद इसे हटा दें।',
  'server.pageSavedSinceOpened': 'आपके इसे खोलने के बाद किसी और ने इस पृष्ठ को सहेजा।',
  'server.noFileUploaded': 'कोई फ़ाइल अपलोड नहीं की गई',
  'server.assetMediaType':
    '"{file}" को सत्यापित करने में त्रुटि। मीडिया प्रकार "{media}" वाली फ़ाइलें संग्रहीत नहीं की जा सकतीं।',
  'server.assetExtension':
    '"{file}" को सत्यापित करने में त्रुटि। रिज़ॉल्व्ड एक्सटेंशन "{extension}" वाली फ़ाइलें संग्रहीत नहीं की जा सकतीं।',
  'server.assetNameRequired': 'एसेट का नाम होना चाहिए।',
  'server.classesNotArray': 'क्लासेज एक ऐरे (array) होनी चाहिए',
  'server.propertiesNotArray': 'प्रॉपर्टीज़ एक ऐरे (array) होनी चाहिए',
  'server.namespaceUriEmpty': 'नेमस्पेस URI खाली नहीं हो सकता।',
  'server.namespaceUriTaken': 'नेमस्पेस URI "{uri}" पहले से लिया जा चुका है।',
  'server.prefixEmpty': 'प्रीफ़िक्स खाली नहीं हो सकता।',
  'server.prefixInvalid': 'प्रीफ़िक्स में केवल अक्षर, संख्याएँ, हाइफ़न और अंडरस्कोर हो सकते हैं।',
  'server.prefixTaken': 'प्रीफ़िक्स "{prefix}" पहले से लिया जा चुका है।',
  'server.prefixReserved': 'प्रीफ़िक्स "{prefix}" आरक्षित है।',
  'server.labelEmpty': 'लेबल खाली नहीं हो सकता।',
  'server.labelTaken': 'लेबल पहले से लिया जा चुका है।',
  'server.localNameEmpty': 'स्थानीय नाम खाली नहीं हो सकता।',
  'server.localNameTaken': 'स्थानीय नाम "{name}" पहले से लिया जा चुका है।',
  'server.vocabularyRequired': 'शब्दावली निर्धारित की जानी चाहिए।',
  'server.duplicateTemplateProperty': '{id} ID के साथ डुप्लिकेट प्रॉपर्टी जोड़ने का प्रयास',
  'server.vocabularyNoFile': 'फ़ाइल आयात रणनीति के लिए कोई फ़ाइल निर्दिष्ट नहीं है।',
  'server.vocabularyNoUrl': 'URL आयात रणनीति के लिए कोई URL निर्दिष्ट नहीं है।',
  'server.vocabularyNotParsed': 'शब्दावली फ़ाइल को पार्स नहीं किया जा सका।',
  'server.vocabularyNotLoaded': 'URL से शब्दावली लोड नहीं की जा सकी।',
  'server.vocabularyNoMembers':
    'कोई क्लास या प्रॉपर्टी नहीं मिली। क्या आप सुनिश्चित हैं कि आपने सही नेमस्पेस URI का उपयोग किया है?',
  'server.vocabularyPermanent': 'एक स्थायी शब्दावली को संपादित नहीं किया जा सकता',
  'server.vocabularyFormatUnknown':
    'फ़ाइल प्रारूप का पता नहीं लगाया जा सका। सूची से प्रारूप चुनें।',
  'server.vocabularyFormatInvalid': '"{format}" शब्दावली फ़ाइल प्रारूप नहीं है।',
  'server.vocabularyUrlScheme': 'केवल http और https URL आयात किए जा सकते हैं।',
  'server.vocabularyUrlPrivate':
    'URL एक निजी या स्थानीय नेटवर्क पते को इंगित करता है, जिसे सर्वर प्राप्त नहीं करता है।',
  'server.vocabularyUrlTooLarge': 'URL पर फ़ाइल {size} से बड़ी है।',
  'server.vocabularyUrlTimeout': 'URL पर सर्वर ने समय पर उत्तर नहीं दिया।',
  'server.vocabularyUrlStatus': 'URL पर सर्वर ने स्थिति {status} के साथ उत्तर दिया।',
  'server.vocabularyChangeStale':
    '"{name}" की तुलना में परिवर्तन हुआ है। फ़ाइल की फिर से तुलना करें।',
  'server.vocabularyMemberMissing': '"{name}" इस शब्दावली का सदस्य नहीं है।',
  'server.nameEmpty': 'नाम खाली नहीं हो सकता।',
  'server.emailTaken': 'ईमेल {email} पहले से लिया जा चुका है।',
  'server.userRoleRequired': 'उपयोगकर्ताओं की एक भूमिका होनी चाहिए।',
  'server.invalidEmail':
    'इनपुट एक वैध ईमेल पता नहीं है। बुनियादी प्रारूप local-part@hostname का उपयोग करें',
  'server.siteTitleRequired': 'एक साइट का शीर्षक होना चाहिए।',
  'server.siteThemeRequired': 'एक साइट का थीम होना चाहिए।',
  'server.siteItemPoolRequired': 'एक साइट में आइटम पूल डेटा होना चाहिए।',
  'server.homepageNotInSite': 'एक होमपेज अपनी पैरेंट साइट से संबंधित होना चाहिए।',
  'server.slugEmpty': 'स्लग खाली नहीं हो सकता।',
  'server.slugInvalid': 'एक स्लग में केवल अक्षर, संख्या, अंडरस्कोर और हाइफ़न हो सकते हैं।',
  'server.slugTaken': 'स्लग "{slug}" पहले से ही लिया जा चुका है।',
  'server.pageSiteRequired': 'एक पेज साइट से संबंधित होना चाहिए।',
  'server.pageTitleRequired': 'एक पेज का शीर्षक होना चाहिए।',
  'server.blockLayoutRequired': 'सभी ब्लॉक्स का लेआउट होना चाहिए।',
  'server.blockDataScalar': 'ब्लॉक डेटा स्केलर वैल्यू नहीं होना चाहिए।',
  'server.siteSettingsNotObject': 'अपेक्षित ऑब्जेक्ट या नल, प्राप्त हुआ {kind}।',
  'server.cssTooLarge':
    'कस्टम CSS: लाइन {line}, कॉलम {column}: कस्टम CSS अधिकतम {max} बाइट्स हो सकती है; यह {bytes} है।',
  'server.cssImport': 'कस्टम CSS: लाइन {line}, कॉलम {column}: @import की अनुमति नहीं है।',
  'server.cssDocument': 'कस्टम CSS: लाइन {line}, कॉलम {column}: @document की अनुमति नहीं है।',
  'server.cssForeignUrl':
    'कस्टम CSS: लाइन {line}, कॉलम {column}: url({url}) की अनुमति नहीं है; एक URL /files/ या /assets/ के अंतर्गत एक पथ होना चाहिए।',
  'server.cssExpression': 'कस्टम CSS: लाइन {line}, कॉलम {column}: expression() की अनुमति नहीं है।',
  'server.cssBehavior': 'कस्टम CSS: लाइन {line}, कॉलम {column}: behavior की अनुमति नहीं है।',
  'server.cssMozBinding': 'कस्टम CSS: लाइन {line}, कॉलम {column}: -moz-binding की अनुमति नहीं है।',
  'server.cssStyleBreakout': 'कस्टम CSS: लाइन {line}, कॉलम {column}: </style की अनुमति नहीं है।',
  'server.cssBadUrl': 'कस्टम CSS: लाइन {line}, कॉलम {column}: यह url( वैध CSS नहीं है।',
  'server.cssUnclosedString': 'कस्टम CSS: लाइन {line}, कॉलम {column}: यह स्ट्रिंग बंद नहीं है।',
  'server.cssStrayClose':
    'कस्टम CSS: लाइन {line}, कॉलम {column}: {bracket} कुछ भी बंद नहीं करता है।',
  'server.navNotArray': 'अमान्य नेविगेशन: नेविगेशन एक ऐरे होना चाहिए',
  'server.navLinkMissingType': 'अमान्य नेविगेशन: लिंक में टाइप (type) अनुपस्थित है',
  'server.navLinkMissingData': 'अमान्य नेविगेशन: लिंक में डेटा (data) अनुपस्थित है',
  'server.navInvalidLinkData': 'अमान्य नेविगेशन: अमान्य लिंक डेटा',
  'server.navPageLinksNotUnique': 'अमान्य नेविगेशन: पेज लिंक्स अद्वितीय होने चाहिए',
  'server.navLinksNotArray': 'अमान्य नेविगेशन: लिंक्स एक ऐरे होने चाहिए',
  'server.navPageLinkMissingPage': 'अमान्य नेविगेशन: पेज लिंक में पेज ID अनुपस्थित है',
  'server.navUrlMissingLabel': 'अमान्य नेविगेशन: URL लिंक में लेबल (label) अनुपस्थित है',
  'server.navUrlMissingUrl': 'अमान्य नेविगेशन: URL लिंक में URL अनुपस्थित है',
  'server.navUrlInvalidCharacter': 'अमान्य नेविगेशन: URL लिंक में अमान्य वर्ण है',
  'server.navUrlInvalidScheme': 'अमान्य नेविगेशन: URL लिंक में अमान्य स्कीम है',
  'server.settingsDenied':
    'वर्तमान उपयोगकर्ता के लिए वैश्विक सेटिंग्स तक पहुँचने के लिए अनुमति अस्वीकृत।',
  'server.settingsNotObject': 'एक ऑब्जेक्ट की अपेक्षा थी।',
  'server.settingUnavailable': 'सेटिंग {key} उपलब्ध नहीं है।',
  'server.settingNotText': 'सेटिंग {key} एक स्ट्रिंग या नल होनी चाहिए।',
  'server.settingNotFlag': 'सेटिंग {key} सत्य, असत्य या नल होनी चाहिए।',
  'server.settingNotNames': 'सेटिंग {key} स्ट्रिंग्स की एक सूची या नल होनी चाहिए।',
  'server.settingNotCount': 'सेटिंग {key} शून्य से ऊपर एक पूर्ण संख्या या शून्य होनी चाहिए।',
  'server.publicationTermsRemoved':
    'एक प्रकाशित शब्द को हटाया या उसका नाम बदला नहीं जा सकता: {terms}। इसके बजाय इसे अप्रचलित (deprecate) करें।',
  'server.publicationNamespaceFixed':
    'एक प्रकाशित शब्दावली अपना नेमस्पेस बनाए रखती है, इसलिए इसके शब्द अपने URI बनाए रखते हैं।',
  'server.publicationNotDeletable':
    'एक प्रकाशित शब्द सूची या शब्दावली को हटाया नहीं जा सकता। इसके बजाय इसके शब्दों को अप्रचलित करें।',
  'server.publicationDeprecationFixed':
    '"{term}" का अप्रचलन प्रकाशित हो चुका है और इसे वापस नहीं लिया जा सकता।',
  'server.publicationItemsList':
    'वस्तुओं की सूची प्रकाशित नहीं की जा सकती। प्रत्येक वस्तु का अपना पता पहले से है।',
  'server.publicationForeignNamespace':
    'नेमस्पेस {namespace} इस संग्रह का नहीं है। इस संग्रह के /vocab/ पते के अंतर्गत एक शब्दावली प्रकाशित करें, या w3id.org या purl.org पर।',
  'server.publicationSlugInvalid':
    'एक पते के नाम में छोटे अक्षर, अंक और एकल हाइफ़न का उपयोग किया जाता है, जो 64 वर्णों तक होता है।',
  'server.publicationSlugTaken': 'पता /vocab/{slug}/ पहले से लिया जा चुका है।',
  'server.publicationNamespaceNotPersistent':
    'इस संग्रह के अलावा अन्य नेमस्पेस w3id.org या purl.org पर होना चाहिए और / या # पर समाप्त होना चाहिए।',
  'server.publicationAddressFixed': 'एक प्रकाशित शब्द सूची या शब्दावली अपना मूल पता बनाए रखती है।',
  'server.publicationTermNotPublished':
    '"{term}" अभी तक प्रकाशित नहीं हुआ है, इसलिए इसे अप्रचलित नहीं किया जा सकता। इसके बजाय इसे हटा दें।',
  'server.publicationUnknownTerm': '"{term}" इस सूची या शब्दावली का शब्द नहीं है।',
  'server.publicationMatchInvalid': 'मिलान एक पूर्ण http या https URI होना चाहिए।',
  'server.publicationMatchTermLists': 'केवल एक शब्द सूची के शब्द ही किसी प्राधिकरण से जुड़ते हैं।',
  'server.publicationTermIriInvalid': 'शब्द URI {uri} एक वैध IRI नहीं है।',
  'server.importAdminsOnly': 'केवल वैश्विक या साइट प्रशासक ही आर्काइव आयात कर सकता है।',
  'server.importNotFound': '{id} आईडी वाला कोई आयात नहीं है।',
  'server.importRequestUnreadable': 'अनुरोध को पढ़ा नहीं जा सकता: {reason}',
  'server.importPlanRefused': 'आयात की सेटिंग्स सहेजी नहीं जा सकतीं: {reason}',
  'server.importStale':
    'किसी और ने इस आयात को आपके द्वारा खोलने के बाद सहेजा है। उनके परिवर्तनों को देखने के लिए इसे पुनः लोड करें।',
  'server.importPlanFrozen':
    'यह आयात पहले ही शुरू हो चुका है, इसलिए इसकी सेटिंग्स नहीं बदल सकतीं। इसे बदलने के लिए एक प्रति बनाएँ।',
  'server.importV1Plan':
    'यह आयात पुराने आयातकर्ता द्वारा बनाया गया था, इसलिए इसे यहाँ खोला या कॉपी नहीं किया जा सकता।',
  'server.importTypeExists':
    'Omeka में "{label}" नाम का एक आइटम प्रकार पहले से मौजूद है। नए प्रकार के लिए दूसरा नाम चुनें।',
  'server.importDeleteNotDraft':
    'केवल वही आयात हटाया जा सकता है जिसे अभी तक जोड़ा नहीं गया है। जो आयात चल चुका है, वह पहले के आयातों में रहता है।',
  'server.importChanged':
    'अंतिम जाँच के बाद से फ़ाइलें या आयात की सेटिंग्स बदल गई हैं। इसे फिर से चलाने से पहले आयात की दोबारा जाँच करें।',
  'server.importBlocked':
    'यह आयात तब तक नहीं चल सकता जब तक जाँच में सूचीबद्ध समस्याएँ ठीक नहीं हो जातीं।',
  'server.importNothingToRun': 'इस आयात में लिखने के लिए कुछ भी नहीं है।',
  'server.importConfirmDelete':
    'यह आयात {count} आइटम, मीडिया और आइटम सेट हटा देता है और इसे पूर्ववत नहीं किया जा सकता। इसे चलाने के लिए उस संख्या की पुष्टि करें।',
  'server.importBusy':
    'एक अन्य आयात चल रहा है या पूर्ववत किया जा रहा है। जब यह समाप्त हो जाए, तब पुनः प्रयास करें।',
  'server.importCannotRun':
    'केवल एक आयात जिसे अभी तक जोड़ा नहीं गया है, जो अंत से पहले रुक गया था, या जो विफलताओं के साथ समाप्त हुआ, वह चल सकता है।',
  'server.importCannotUndo':
    'केवल वही आयात पूर्ववत किया जा सकता है जो पूरा हो चुका है या अंत से पहले रुक गया था।',
  'server.importNotRunning': 'यह आयात नहीं चल रहा है।',
  'server.importNoRoot': '"{root}" नाम का कोई आर्काइव फ़ोल्डर नहीं है।',
  'server.importOutsideRoot': '"{path}" फ़ोल्डर आर्काइव फ़ोल्डर के बाहर है।',
  'server.importFolderMissing': 'आर्काइव में "{path}" फ़ोल्डर नहीं है।',
  'server.importNotAFolder': '"{path}" एक फ़ाइल है, फ़ोल्डर नहीं।',
  'server.importScanBusy':
    'यह आयात चल रहा है या पूर्ववत किया जा रहा है। यह समाप्त होने के बाद फ़ाइलों को फिर से देखें।',
  'server.importNoLeftFile':
    'इस आयात में अभी तक छोड़ी गई फ़ाइलों की सूची नहीं है। यह किसी आयात के समाप्त होने पर लिखी जाती है।',
  'server.importJobNotStarted': 'आयात शुरू नहीं हो सका: {reason}',
  'server.importListColumnUnknown':
    'सूची में "{column}" कॉलम नहीं हो सकता। कोई प्रॉपर्टी, आइटम सेट, आइटम टाइप या विजिबिलिटी चुनें।',
  'server.importListOneFile': 'एक सूची फ़ाइल फॉर्म में भेजें।',
  'server.importListType':
    '"{name}" एक .csv, .tsv, .xlsx, .xls या .ods फ़ाइल नहीं है। सूची को इनमें से किसी एक के रूप में सहेजें, फिर इसे फिर से भेजें।',
  'server.importListTooLarge': 'सूची 50 MB से बड़ी है।',
  'server.importListUnread': 'सूची नहीं पढ़ी जा सकती: {reason}',
  'server.importListNoIdentifier':
    'सूची में आइडेंटिफ़ायर कॉलम नहीं है, इसलिए इसकी पंक्तियाँ अपने आइटम नहीं ढूंढ सकतीं। आपने जो सूची डाउनलोड की थी, उसका आइडेंटिफ़ायर कॉलम बनाए रखें।',
  'server.importPairsColumns':
    'सूची में Row कॉलम और Files कॉलम चाहिए। आपने जो सूची डाउनलोड की थी, उसके दोनों कॉलम रहने दें।',
  'server.importFileUnreadable': 'फ़ाइल "{path}" नहीं पढ़ी जा सकी: {reason}',
  'server.importFileRefused': 'फ़ाइल "{path}" नहीं जोड़ी गई: {reason}',
  'server.importFileNotStored': 'फ़ाइल "{path}" संग्रहीत नहीं की जा सकी: {reason}',
  'server.importSourceRefused': '"{source}" नहीं जोड़ा गया: {reason}',
  'server.importMediaNoSource': 'इस मीडिया में जोड़ने के लिए न कोई फ़ाइल बताई गई है, न कोई पता।',
  'server.importMediaNoItem': 'इस मीडिया का आइटम नहीं जोड़ा गया, इसलिए मीडिया भी नहीं जोड़ा गया।',
  'server.importResourceGone':
    'यह रिकॉर्ड जिस आइटम, मीडिया या आइटम सेट को बदलता है, वह अब Omeka में नहीं है।',
  'server.importRecordNotSaved': 'इस रिकॉर्ड को सहेजा नहीं जा सका: {reason}',
  'server.importLinksNotSaved': 'इस रिकॉर्ड के लिंक सहेजे नहीं जा सके: {reason}',
  'server.importRecordNotDeleted': 'इसे हटाया नहीं गया: {reason}',
  'server.importDeleteNotUndone':
    'जो आयात आइटम, मीडिया या आइटम सेट को हटाता है, उसे पूर्ववत नहीं किया जा सकता।',
  'server.importSitesNotJoined':
    'यह आइटम जोड़ा गया, लेकिन यह अपनी साइटों में शामिल नहीं हो सका: {reason}',
  'server.importUndoEdited': 'आयात द्वारा इसे लिखे जाने के बाद इसे संपादित किया गया।',
  'server.importUndoLinked': 'इस आयात के बाहर का कोई आइटम, मीडिया या आइटम सेट इससे जुड़ा है।',
  'server.importUndoMediaAdded': 'इस आयात के बाहर से इस आइटम में मीडिया जोड़ा गया।',
  'server.importUndoMembersAdded': 'इस आयात के बाहर के आइटम इस आइटम सेट में रखे गए।',
  'server.importUndoLaterImport': 'इस आयात के बाद आयात {import} ने इसे बदल दिया।',
  'server.importUndoInUse':
    'इस आयात के बाहर के आइटम, मीडिया, आइटम सेट या आइटम प्रकार इसका उपयोग करते हैं।',
  'server.importUndoItemKept': 'इस मीडिया का आइटम रखा जाता है, इसलिए मीडिया भी रखा जाता है।',
  'server.importUndoMemberKept':
    'इस आइटम सेट का एक आइटम रखा जाता है, इसलिए आइटम सेट भी रखा जाता है।',
  'server.importUndoStopped': 'पूर्ववत करना अंत से पहले ही रुक गया: {reason}',
  'server.importUploadOwnerOnly':
    'केवल इस आयात को शुरू करने वाले व्यक्ति, या एक वैश्विक प्रशासक, इसमें फ़ाइलें जोड़ सकते हैं।',
  'server.importUploadNotDraft':
    'फ़ाइलें केवल आयात शुरू होने से पहले जोड़ी जा सकती हैं। अधिक फ़ाइलें जोड़ने के लिए, इस आयात की प्रतिलिपि बनाएँ।',
  'server.importUploadPath': 'फ़ाइल का नाम "{path}" उपयोग नहीं किया जा सकता: {reason}।',
  'server.importUploadTooLarge':
    'यह फ़ाइल {limit} bytes से बड़ी है, जो एक आयात द्वारा ली जाने वाली सबसे बड़ी फ़ाइल है।',
  'server.importUploadFull':
    'इस आयात की फ़ाइलें {limit} bytes से अधिक हो जाएँगी, जो एक आयात द्वारा रखी जाने वाली अधिकतम फ़ाइल है। कुछ फ़ाइलें हटाएँ, या उनकी वर्तमान प्रक्रिया पूरी होने दें, और पुनः प्रयास करें।',
  'server.importUploadChunkTooLarge':
    'यह अनुरोध {limit} bytes से अधिक डेटा ले जाता है। फ़ाइल को छोटे टुकड़ों में भेजें।',
  'server.importUploadInProgress':
    '"{path}" अभी भी रास्ते में है। फिर से शुरू करने के बजाय उस अपलोड के साथ आगे बढ़ें।',
  'server.importUploadNotFound': 'इस आयात में आईडी "{upload}" वाला कोई अपलोड नहीं है।',
  'server.importUploadOffset':
    'सर्वर ने इस फ़ाइल के {received} बाइट्स प्राप्त कर लिए हैं। उनके बाद आने वाले बाइट्स भेजें।',
  'server.importUploadPastLength': 'ये बाइट्स इस अपलोड द्वारा घोषित आकार से अधिक हैं।',
  'server.importUploadLength': 'फ़ाइल के आकार को बाइट्स में भेजें, इसके साथ Upload-Length भेजें।',
  'server.importUploadMetadata':
    'फ़ाइल का नाम "filename" के रूप में और इसके नाम को base64 में "name" के रूप में भेजें।',
  'server.importUploadOffsetMissing':
    'पहले से भेजे जा चुके बाइट्स की संख्या के साथ Upload-Offset भेजें।',
  'server.importUploadContentType':
    'Content-Type application/offset+octet-stream के साथ बाइट्स भेजें।',
  'server.importUploadVersion':
    'यह सर्वर रिज़्यूमेबल अपलोड प्रोटोकॉल के संस्करण 1.0.0 का समर्थन करता है। Tus-Resumable: 1.0.0 भेजें।',
  'server.importUploadInterrupted':
    'अनुरोध समाप्त होने से पहले बाइट्स आना बंद हो गए, इसलिए उनमें से किसी को भी सहेजा नहीं गया। ऑफसेट मांगें और उन्हें फिर से भेजें।',
  'server.importZipEscapes':
    '"{name}" ज़िप में अपने फ़ोल्डर के बाहर होगा, इसलिए इसे छोड़ दिया गया।',
  'server.importZipLink': '"{name}" ज़िप में एक लिंक है, इसलिए इसे छोड़ दिया गया।',
  'server.importZipUnusable':
    'ज़िप में "{name}" का नाम उपयोग के योग्य नहीं है ({reason}), इसलिए इसे बाहर छोड़ दिया गया।',
  'server.importZipUnsupported':
    'ज़िप में "{name}" एन्क्रिप्टेड है या ऐसे ढंग से पैक किया गया है जिसे रोमका एस नहीं खोल सकता, इसलिए इसे बाहर छोड़ दिया गया।',
  'server.importZipRenamed':
    '"{name}" ज़िप में एक नाम है जो बड़े अक्षरों को अनदेखा करने पर पहले की फ़ाइल के नाम जैसा है, इसलिए इसे "{renamed}" के रूप में जोड़ा गया।',
  'server.importZipBomb':
    'ज़िप को अस्वीकार कर दिया गया: "{name}" अपने संकुचित आकार के 100 गुना से अधिक अनपैक होता है। इससे कुछ भी नहीं जोड़ा गया।',
  'server.importZipFull':
    'ज़िप को अस्वीकार कर दिया गया: इसकी फ़ाइलें {limit} बाइट्स से अधिक होंगी, जो एक आयात में रखी जा सकने वाली अधिकतम संख्या है। इससे कुछ भी नहीं जोड़ा गया।',
  'server.importZipTooMany':
    'ज़िप को अस्वीकार कर दिया गया: इसमें {count} फ़ाइलें हैं, जो 100,000 से अधिक हैं। इससे कुछ भी नहीं जोड़ा गया।',
  'server.importZipFileTooLarge':
    'ज़िप को अस्वीकार कर दिया गया: "{name}" {limit} बाइट्स से बड़ा है, जो एक आयात द्वारा लिया जा सकने वाला सबसे बड़ा फ़ाइल आकार है। इससे कुछ भी नहीं जोड़ा गया।',
  'server.importZipUnreadable':
    'ज़िप को अस्वीकार कर दिया गया: इसे पढ़ा नहीं जा सका ({reason})। इससे कुछ भी नहीं जोड़ा गया।',
  'server.importZipUnpacked': 'ज़िप को अनपैक किया गया: {count} फ़ाइलें जोड़ी गईं।',
  'server.importSweepDone': '{imports} आयात की फ़ाइलों को हटा दिया गया, और {uploads} अधूरे अपलोड।',
  'server.importAccessFailed': 'मीडिया {media} के लिए कोई प्लेएबल कॉपी कतारबद्ध नहीं थी: {reason}',
  'server.importAccessUnread': 'इस आयात के लिए कोई प्लेएबल कॉपी कतारबद्ध नहीं थी: {reason}',
  'server.mediaUploadNotFound': 'इस खाते में "{upload}" आईडी के साथ कोई स्टेज्ड अपलोड नहीं है।',
  'server.mediaUploadBusy':
    'यह अपलोड "{upload}" अभी जोड़ा जा रहा है। प्रतीक्षा करें और पुनः प्रयास करें।',
  'server.mediaUploadIncomplete':
    'इस अपलोड में इसके {length} बाइट्स का {received} हिस्सा है। इसे जोड़ने से पहले बाकी हिस्सा भेजें।',
  'server.mediaUploadIngester': 'एक स्टेज्ड अपलोड अपलोड इनगेस्टर के साथ जोड़ा गया है।',
  'server.noDerivativeToRetry': 'इस मीडिया की कोई विफल प्रति नहीं है जिसे फिर से बनाया जा सके।',
  'server.mediaUploadInline':
    'एक स्टेज्ड अपलोड अपनी मीडिया क्रिएशन के साथ जोड़ा गया है, न कि किसी आइटम के अंदर।',
  'server.uploadTooLarge':
    'यह फ़ाइल {limit} बाइट्स से बड़ी है, जो मीडिया अपलोड द्वारा लिया जा सकने वाला सबसे बड़ा फ़ाइल आकार है।',
  'server.mediaLinkInvalid': 'लिंक "{url}" http या https पता नहीं है।',
  'server.mediaLinkUnreadable': 'लिंक "{url}" पढ़ा नहीं जा सका: {reason}',
  'server.mediaLinkStatus': 'लिंक "{url}" ने {status} {reason} का उत्तर दिया।',
  'server.uploadItem': 'अपलोड की आइटम आईडी को एक ऐसी आइटम का नाम देना चाहिए जो मौजूद हो।',
  'server.mediaUploadSweepDone':
    '{count} स्टेज्ड अपलोड्स का रखा गया समय निकल गया और उन्हें हटा दिया गया।',
};
