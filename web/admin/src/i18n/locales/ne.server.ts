/**
 * The Nepali server messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `ServerCatalogue`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ServerCatalogue } from '../serverMessages';

export const neServer: ServerCatalogue = {
  'server.methodNotAllowed': 'विधि अनुमति छैन',
  'server.notFound': 'फेला परेन',
  'server.badRequest': 'खराब अनुरोध',
  'server.requestTimeout': 'अनुरोध समय समाप्त',
  'server.bodyTooLarge': 'अनुरोध बडी धेरै ठुलो छ',
  'server.internalError': 'आन्तरिक सर्भर त्रुटि',
  'server.tooManyRequests': 'धेरै अनुरोधहरू',
  'server.unknownResource': 'API ले "{resource}" संसाधनलाई समर्थन गर्दैन।',
  'server.adapterNotImplemented': '{resource} एडाप्टरले {operation} अपरेसन लागू गर्दैन।',
  'server.endpointNotImplemented': '{endpoint} एन्डपोइन्ट अहिलेसम्म लागू गरिएको छैन।',
  'server.permissionDenied':
    'वर्तमान प्रयोगकर्तालाई {operation} {resource} संसाधन गर्न अनुमति छैन।',
  'server.entityNotFound': '{class} संस्था {criteria} मापदण्डका साथ फेला परेन',
  'server.jsonSyntax': 'JSON: वाक्य संरचना त्रुटि',
  'server.jsonNotContainer': 'JSON: सामग्री अब्जेक्ट वा एरे हुनुपर्छ।',
  'server.invalidContentType':
    'अमान्य Content-Type हेडर। "application/json" को अपेक्षा गरिएको, "{received}" प्राप्त भयो।',
  'server.bodyNotObject': 'अनुरोध बडी JSON अब्जेक्ट हुनुपर्छ।',
  'server.siteItemsOneSelector': 'नाम कि त "query", वा "items", "item_sets" वा दुवै हुनुपर्छ',
  'server.invalidCredentials': 'इमेल वा पासवर्ड अमान्य छ',
  'server.inactiveAccount': 'प्रयोगकर्ता सक्रिय छैन।',
  'server.unknownKey': 'अमान्य पासवर्ड सिर्जना कुञ्जी।',
  'server.authenticationRequired': 'प्रमाणीकरण आवश्यक छ।',
  'server.invalidPassword': 'पासवर्ड अमान्य छ',
  'server.crossOriginRefused': 'सेसन-प्रमाणित लेख कन्फिगर गरिएको बेस URL बाट आउनुपर्छ।',
  'server.propertyNotRecognized': 'गुण "{term}" पहिचान गरिएको छैन।',
  'server.resourceNotFound': 'id {id} को साथ स्रोत फेला परेन।',
  'server.templateRequiresValue': '"{template}" संसाधन टेम्प्लेटलाई "{property}" मान आवश्यक पर्छ',
  'server.itemSetsNotArray': 'वस्तु सेटहरू एउटा एरे हुनुपर्छ',
  'server.mediaNotArray': 'मिडिया एउटा एरे हुनुपर्छ',
  'server.mediaNotThisItem': 'मिडिया यस वस्तुको हुनुपर्छ।',
  'server.primaryMediaNotThisItem': 'प्राथमिक मिडिया यस वस्तुको हुनुपर्छ।',
  'server.mediaIngesterRequired': 'मिडियाले एउटा इन्जेस्टर सेट गर्नुपर्छ।',
  'server.mediaIngesterInvalid': 'मिडियाले एउटा मान्य इन्जेस्टर सेट गर्नुपर्छ।',
  'server.mediaItemRequired': 'मिडिया कुनै वस्तुको हुनुपर्छ।',
  'server.jobHeldByImport':
    'यो कार्य चलिरहेको वा पूर्ववत हुँदै गरेको आर्काइभ आयातको हो। आयात समाप्त भएपछि यसलाई हटाउनुहोस्।',
  'server.pageSavedSinceOpened': 'कसैले यो पृष्ठ तपाईँले खोल्नुभएको समयपछि सुरक्षित गरेको छ।',
  'server.noFileUploaded': 'कुनै फाइल अपलोड गरिएको छैन',
  'server.assetMediaType':
    '"{file}" को प्रमाणीकरण गर्दा त्रुटि। मिडिया प्रकार "{media}" भएका फाइलहरू भण्डारण गर्न सकिँदैन।',
  'server.assetExtension':
    '"{file}" को प्रमाणीकरण गर्दा त्रुटि। समाधान गरिएको विस्तार "{extension}" भएका फाइलहरू भण्डारण गर्न सकिँदैन।',
  'server.assetNameRequired': 'एसेटको नाम हुनुपर्छ।',
  'server.classesNotArray': 'क्लासहरू एउटा एरे हुनुपर्छ',
  'server.propertiesNotArray': 'प्रोपर्टीहरू एउटा एरे हुनुपर्छ',
  'server.namespaceUriEmpty': 'नेमस्पेस URI खाली हुन सक्दैन।',
  'server.namespaceUriTaken': 'नेमस्पेस URI "{uri}" पहिले नै लिइएको छ।',
  'server.prefixEmpty': 'प्रिफिक्स खाली हुन सक्दैन।',
  'server.prefixInvalid': 'प्रिफिक्समा अक्षर, सङ्ख्या, हाइफन र अन्डरस्कोर मात्र हुन सक्छ।',
  'server.prefixTaken': 'प्रिफिक्स "{prefix}" पहिले नै लिइएको छ।',
  'server.prefixReserved': 'प्रिफिक्स "{prefix}" आरक्षित छ।',
  'server.labelEmpty': 'लेबल खाली हुन सक्दैन।',
  'server.labelTaken': 'लेबल पहिले नै लिइएको छ।',
  'server.localNameEmpty': 'स्थानीय नाम खाली हुन सक्दैन।',
  'server.localNameTaken': 'स्थानीय नाम "{name}" पहिले नै लिइएको छ।',
  'server.vocabularyRequired': 'शब्दावली सेट गर्नुपर्छ।',
  'server.duplicateTemplateProperty': 'ID {id} भएको दोहोरिएको प्रोपर्टी थप्ने प्रयास',
  'server.vocabularyNoFile': 'फाइल आयात रणनीतिका लागि कुनै फाइल निर्दिष्ट गरिएको छैन।',
  'server.vocabularyNoUrl': 'URL आयात रणनीतिका लागि कुनै URL निर्दिष्ट गरिएको छैन।',
  'server.vocabularyNotParsed': 'शब्दावली फाइल पार्स गर्न सकिएन।',
  'server.vocabularyNotLoaded': 'URL बाट शब्दावली लोड गर्न सकिएन।',
  'server.vocabularyNoMembers':
    'कुनै क्लास वा प्रोपर्टी फेला परेन। के तपाईँले सही नेमस्पेस URI प्रयोग गर्नुभएको छ?',
  'server.vocabularyPermanent': 'स्थायी शब्दावली सम्पादन गर्न सकिँदैन',
  'server.vocabularyFormatUnknown': 'फाइल ढाँचा पत्ता लगाउन सकिएन। सूचीबाट ढाँचा छान्नुहोस्।',
  'server.vocabularyFormatInvalid': '"{format}" शब्दावली फाइल ढाँचा होइन।',
  'server.vocabularyUrlScheme': 'केवल http र https URL हरू आयात गर्न सकिन्छ।',
  'server.vocabularyUrlPrivate':
    'URL ले एउटा निजी वा स्थानीय नेटवर्क ठेगानालाई सङ्केत गर्दछ, जसलाई सर्भरले प्राप्त गर्दैन।',
  'server.vocabularyUrlTooLarge': 'URL मा रहेको फाइल {size} भन्दा ठुलो छ।',
  'server.vocabularyUrlTimeout': 'URL मा रहेको सर्भरले समयमै जवाफ दिएन।',
  'server.vocabularyUrlStatus': 'URL मा रहेको सर्भरले {status} स्थितिमा जवाफ दियो।',
  'server.vocabularyChangeStale': '"{name}" तुलनाबाट परिवर्तन भएको छ। फाइल फेरि तुलना गर्नुहोस्।',
  'server.vocabularyMemberMissing': '"{name}" यो शब्दावलीको सदस्य होइन।',
  'server.nameEmpty': 'नाम खाली हुन सक्दैन।',
  'server.emailTaken': 'इमेल {email} पहिले नै लिइएको छ।',
  'server.userRoleRequired': 'प्रयोगकर्ताहरूलाई एउटा भूमिका तोकिनुपर्छ।',
  'server.invalidEmail':
    'इनपुट एउटा वैध इमेल ठेगाना होइन। आधारभूत ढाँचा local-part@hostname प्रयोग गर्नुहोस्',
  'server.siteTitleRequired': 'एउटा साइटको शीर्षक हुनुपर्छ।',
  'server.siteThemeRequired': 'एउटा साइटको थिम हुनुपर्छ।',
  'server.siteItemPoolRequired': 'एउटा साइटमा item pool data हुनुपर्छ।',
  'server.homepageNotInSite': 'एउटा होमपेज यसको अभिभावक साइटको हुनुपर्छ।',
  'server.slugEmpty': 'स्लग खाली हुन सक्दैन।',
  'server.slugInvalid': 'स्लगमा अक्षर, सङ्ख्या, अन्डरस्कोर र हाइफन मात्र हुन सक्छ।',
  'server.slugTaken': 'स्लग "{slug}" पहिले नै लिइएको छ।',
  'server.pageSiteRequired': 'एउटा पृष्ठ साइटको हुनुपर्छ।',
  'server.pageTitleRequired': 'एउटा पृष्ठको शीर्षक हुनुपर्छ।',
  'server.blockLayoutRequired': 'सबै ब्लकहरूको लेआउट हुनुपर्छ।',
  'server.blockDataScalar': 'ब्लक डेटा स्केलर मान हुनु हुँदैन।',
  'server.siteSettingsNotObject': 'अपेक्षित अब्जेक्ट वा नल, {kind} प्राप्त भयो।',
  'server.cssTooLarge':
    'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: अनुकूलित CSS बढीमा {max} बाइट्समा हुन सक्छ; यो {bytes} हो।',
  'server.cssImport': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: @import अनुमति छैन।',
  'server.cssDocument': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: @document अनुमति छैन।',
  'server.cssForeignUrl':
    'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: url({url}) अनुमति छैन; एउटा URL /files/ वा /assets/ अन्तर्गतको पथ हुनुपर्छ।',
  'server.cssExpression': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: expression() अनुमति छैन।',
  'server.cssBehavior': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: behavior अनुमति छैन।',
  'server.cssMozBinding': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: -moz-binding अनुमति छैन।',
  'server.cssStyleBreakout': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: </style अनुमति छैन।',
  'server.cssBadUrl': 'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: यो url( मान्य CSS होइन।',
  'server.cssUnclosedString':
    'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: यो स्ट्रिङ बन्द गरिएको छैन।',
  'server.cssStrayClose':
    'अनुकूलित CSS: लाइन {line}, स्तम्भ {column}: {bracket} ले केही पनि बन्द गर्दैन।',
  'server.navNotArray': 'अमान्य नेभिगेसन: नेभिगेसन एरे हुनुपर्छ',
  'server.navLinkMissingType': 'अमान्य नेभिगेसन: लिङ्कको प्रकार छुटेको',
  'server.navLinkMissingData': 'अमान्य नेभिगेसन: लिङ्कको डेटा छुटेको',
  'server.navInvalidLinkData': 'अमान्य नेभिगेसन: अमान्य लिङ्क डेटा',
  'server.navPageLinksNotUnique': 'अमान्य नेभिगेसन: पृष्ठ लिङ्कहरू अद्वितीय हुनुपर्छ',
  'server.navLinksNotArray': 'अमान्य नेभिगेसन: लिङ्कहरू एरे हुनुपर्छ',
  'server.navPageLinkMissingPage': 'अमान्य नेभिगेसन: पृष्ठ लिङ्कको पेज ID छुटेको',
  'server.navUrlMissingLabel': 'अमान्य नेभिगेसन: URL लिङ्कको लेबल छुटेको',
  'server.navUrlMissingUrl': 'अमान्य नेभिगेसन: URL लिङ्कको URL छुटेको',
  'server.navUrlInvalidCharacter': 'अमान्य नेभिगेसन: URL लिङ्कमा अमान्य क्यारेक्टर छ',
  'server.navUrlInvalidScheme': 'अमान्य नेभिगेसन: URL लिङ्कको स्किम अमान्य छ',
  'server.settingsDenied': 'वर्तमान प्रयोगकर्तालाई ग्लोबल सेटिङहरू पहुँच गर्न अनुमति छैन।',
  'server.settingsNotObject': 'एउटा अब्जेक्ट अपेक्षित थियो।',
  'server.settingUnavailable': 'सेटिङ {key} उपलब्ध छैन।',
  'server.settingNotText': 'सेटिङ {key} स्ट्रिङ वा नल हुनुपर्छ।',
  'server.settingNotFlag': 'सेटिङ {key} सत्य, असत्य वा नल हुनुपर्छ।',
  'server.settingNotNames': 'सेटिङ {key} स्ट्रिङहरूको सूची वा नल हुनुपर्छ।',
  'server.settingNotCount': 'सेटिङ {key} शून्यभन्दा माथिको पूर्ण सङ्ख्या वा शून्य हुनुपर्छ।',
  'server.publicationTermsRemoved':
    'प्रकाशित शब्दलाई हटाउन वा पुन: नामकरण गर्न सकिँदैन: {terms}। यसलाई यसको सट्टा डिप्रिकेट गर्नुहोस्।',
  'server.publicationNamespaceFixed':
    'प्रकाशित शब्दावलीले यसको नेमस्पेस कायम राख्छ, त्यसैले यसका शब्दहरूले तिनीहरूको URIs कायम राख्छन्।',
  'server.publicationNotDeletable':
    'प्रकाशित शब्द सूची वा शब्दावली मेटाउन सकिँदैन। यसको सट्टा यसका शब्दहरू डिप्रिकेट गर्नुहोस्।',
  'server.publicationDeprecationFixed':
    '"{term}" को डिप्रिकेसन प्रकाशित भइसकेको छ र यसलाई फिर्ता लिन सकिँदैन।',
  'server.publicationItemsList':
    'वस्तुहरूको सूची प्रकाशित गर्न सकिँदैन। प्रत्येक वस्तुको पहिले नै आफ्नै ठेगाना छ।',
  'server.publicationForeignNamespace':
    'नेमस्पेस {namespace} यो आर्काइभको होइन। यस आर्काइभको /vocab/ ठेगाना अन्तर्गत एउटा शब्दावली प्रकाशित गर्नुहोस्, वा w3id.org वा purl.org मा।',
  'server.publicationSlugInvalid':
    'एउटा ठेगानाको नामले सानो अक्षर, अङ्क र एउटा हाइफन प्रयोग गर्छ, जसमा 64 वटा क्यारेक्टरसम्म हुन्छ।',
  'server.publicationSlugTaken': 'ठेगाना /vocab/{slug}/ पहिले नै लिइएको छ।',
  'server.publicationNamespaceNotPersistent':
    'यो आर्काइभको नेमस्पेस बाहेक अन्य नेमस्पेस w3id.org वा purl.org मा हुनुपर्छ र / वा # मा समाप्त हुनुपर्छ।',
  'server.publicationAddressFixed':
    'प्रकाशित शब्द सूची वा शब्दावलीले यसलाई पहिलो पटक प्रकाशित गरिएको ठेगानालाई कायम राख्छ।',
  'server.publicationTermNotPublished':
    '"{term}" अझै प्रकाशित भएको छैन, त्यसैले यसलाई डिप्रिकेट गर्न सकिँदैन। यसको सट्टा यसलाई हटाउनुहोस्।',
  'server.publicationUnknownTerm': '"{term}" यो सूची वा शब्दावलीको शब्द होइन।',
  'server.publicationMatchInvalid': 'मिलान (match) एउटा पूर्ण http वा https URI हुनुपर्छ।',
  'server.publicationMatchTermLists': 'केवल एउटा शब्द सूचीको शब्दले एउटा आधिकारिकतालाई लिङ्क गर्छ।',
  'server.publicationTermIriInvalid': 'शब्द URI {uri} एउटा मान्य IRI होइन।',
  'server.importAdminsOnly':
    'केवल एक विश्वव्यापी वा साइट प्रशासकले मात्र एउटा आर्काइभ आयात गर्न सक्छन्।',
  'server.importNotFound': '{id} आइडी भएको कुनै आयात छैन।',
  'server.importRequestUnreadable': 'अनुरोध पढ्न सकिँदैन: {reason}',
  'server.importPlanRefused': 'आयातको सेटिङहरू सुरक्षित गर्न सकिँदैन: {reason}',
  'server.importStale':
    'कसैले यसलाई तपाईँले खोलेपछि यस आयातलाई सुरक्षित गर्‍यो। यसका परिवर्तनहरू हेर्नका लागि यसलाई पुनः लोड गर्नुहोस्।',
  'server.importPlanFrozen':
    'यो आयात पहिले नै सुरु भइसकेको छ, त्यसैले यसको सेटिङहरू परिवर्तन गर्न सकिँदैन। यसलाई परिवर्तन गर्नका लागि प्रतिलिपि बनाउनुहोस्।',
  'server.importV1Plan':
    'यो आयात पुरानो आयातकर्ताद्वारा गरिएको हो, त्यसैले यसलाई यहाँ खोल्न वा प्रतिलिपि गर्न सकिँदैन।',
  'server.importTypeExists':
    'Omeka मा पहिले नै "{label}" नामको एउटा वस्तु प्रकार छ। नयाँ प्रकारका लागि अर्को नाम छान्नुहोस्।',
  'server.importDeleteNotDraft':
    'केवल अहिलेसम्म नथपिएको आयात मात्र मेट्न सकिन्छ। चलिसकेको आयात अघिल्ला आयातहरूमा रहन्छ।',
  'server.importChanged':
    'अन्तिम जाँचपछि फाइलहरू वा आयातको सेटिङहरू परिवर्तन भएका छन्। यसलाई चलाउनु अघि आयातलाई पुनः जाँच गर्नुहोस्।',
  'server.importBlocked':
    'यो आयात तबसम्म चल्न सक्दैन जबसम्म जाँच सूचीमा रहेका समस्याहरू समाधान हुँदैनन्।',
  'server.importNothingToRun': 'यस आयातमा लेख्नका लागि केही छैन।',
  'server.importConfirmDelete':
    'यो आयातले {count} वटा वस्तु, मिडिया र वस्तु सेटहरू मेटाउँछ र यसलाई पूर्ववत गर्न सकिँदैन। यसलाई चलाउनका लागि त्यो सङ्ख्या पुष्टि गर्नुहोस्।',
  'server.importBusy':
    'अर्को आयात चल्दैछ वा पूर्ववत भइरहेको छ। यो समाप्त भएपछि पुनः प्रयास गर्नुहोस्।',
  'server.importCannotRun':
    'केवल एउटा आयात जुन अहिलेसम्म थपिएको छैन, जुन अन्त्य हुनु अघि रोकिएको थियो, वा जुन असफलताका साथ समाप्त भएको थियो, त्यसले मात्र चल्न सक्छ।',
  'server.importCannotUndo':
    'केवल एउटा आयात जुन समाप्त भएको छ, वा जुन अन्त्य हुनु अघि रोकिएको थियो, त्यसलाई मात्र पूर्ववत गर्न सकिन्छ।',
  'server.importNotRunning': 'यो आयात चल्दैछैन।',
  'server.importNoRoot': '"{root}" नामको कुनै आर्काइभ फोल्डर छैन।',
  'server.importOutsideRoot': '"{path}" फोल्डर आर्काइभ फोल्डरको बाहिर छ।',
  'server.importFolderMissing': 'आर्काइभमा "{path}" फोल्डर छैन।',
  'server.importNotAFolder': '"{path}" एउटा फाइल हो, फोल्डर होइन।',
  'server.importScanBusy':
    'यो आयात चल्दैछ वा पूर्ववत भइरहेको छ। यो समाप्त भएपछि फाइलहरू फेरि हेर्नुहोस्।',
  'server.importNoLeftFile':
    'यो आयातमा अहिलेसम्म बाहिर राखिएका फाइलहरूको सूची छैन। यो तब लेखिन्छ जब एउटा रन समाप्त हुन्छ।',
  'server.importJobNotStarted': 'आयात सुरु हुन सकेन: {reason}',
  'server.importListColumnUnknown':
    'सूचीमा "{column}" कोलम हुन सक्दैन। कुनै प्रोपर्टी, आइटम सेट, आइटम टाइप वा भिजुअलिटी छान्नुहोस्।',
  'server.importListOneFile': 'फारममा एउटा सूची फाइल पठाउनुहोस्।',
  'server.importListType':
    '"{name}" .csv, .tsv, .xlsx, .xls वा .ods फाइल होइन। यस सूचीलाई यी मध्ये कुनै एकको रूपमा सुरक्षित गर्नुहोस्, त्यसपछि यसलाई पुनः पठाउनुहोस्।',
  'server.importListTooLarge': 'सूची 50 MB भन्दा ठुलो छ।',
  'server.importListUnread': 'सूची पढ्न सकिएन: {reason}',
  'server.importListNoIdentifier':
    'सूचीमा कुनै आइडेन्टिफायर कोलम छैन, त्यसैले यसका पङ्क्तिहरूले आफ्ना वस्तुहरू फेला पार्न सक्दैनन्। तपाईँले डाउनलोड गर्नुभएको सूचीको आइडेन्टिफायर कोलम सुरक्षित राख्नुहोस्।',
  'server.importPairsColumns':
    'सूचीमा Row र Files स्तम्भ आवश्यक छन्। तपाईँले डाउनलोड गर्नुभएको सूचीका दुवै स्तम्भ राख्नुहोस्।',
  'server.importFileUnreadable': 'फाइल "{path}" पढ्न सकिएन: {reason}',
  'server.importFileRefused': 'फाइल "{path}" थपिएन: {reason}',
  'server.importFileNotStored': 'फाइल "{path}" भण्डारण गर्न सकिएन: {reason}',
  'server.importSourceRefused': '"{source}" थपिएन: {reason}',
  'server.importMediaNoSource': 'यस मिडियामा थप्न कुनै फाइल वा ठेगाना तोकिएको छैन।',
  'server.importMediaNoItem': 'यस मिडियाको वस्तु थपिएन, त्यसैले मिडिया पनि थपिएन।',
  'server.importResourceGone':
    'यो रेकर्ड परिवर्तन गर्ने मिडिया, वस्तु वा वस्तु सेट अब Omeka मा छैन।',
  'server.importRecordNotSaved': 'यो रेकर्ड सुरक्षित गर्न सकिएन: {reason}',
  'server.importLinksNotSaved': 'यस रेकर्डका लिङ्कहरू सुरक्षित गर्न सकिएन: {reason}',
  'server.importRecordNotDeleted': 'यो मेटाइएन: {reason}',
  'server.importDeleteNotUndone':
    'जसले वस्तु, मिडिया वा वस्तु सेट मेटायो, त्यस आयातलाई पूर्ववत गर्न सकिँदैन।',
  'server.importSitesNotJoined':
    'यो वस्तु थपियो, तर यसलाई यसका साइटहरूमा समावेश गर्न सकिएन: {reason}',
  'server.importUndoEdited': 'आयातले यो लेखेपछि यसलाई सम्पादन गरिएको हो।',
  'server.importUndoLinked':
    'यस आयात बाहिरको कुनै वस्तु, मिडिया वा वस्तु सेटले यसलाई लिङ्क गरेको छ।',
  'server.importUndoMediaAdded': 'यस आयात बाहिरबाट यस वस्तुमा मिडिया थपिएको छ।',
  'server.importUndoMembersAdded': 'यस आयात बाहिरका वस्तुहरू यस वस्तु सेटमा राखिएका छन्।',
  'server.importUndoLaterImport': 'यस आयातपछि आयात {import} ले यसलाई परिवर्तन गर्‍यो।',
  'server.importUndoInUse':
    'यस आयात बाहिरका वस्तु, मिडिया, वस्तु सेट वा वस्तु प्रकारहरूले यसको प्रयोग गर्छन्।',
  'server.importUndoItemKept': 'यस मिडियाको वस्तु सुरक्षित छ, त्यसैले मिडिया पनि सुरक्षित छ।',
  'server.importUndoMemberKept':
    'यस वस्तु सेटको एउटा वस्तु सुरक्षित छ, त्यसैले वस्तु सेट पनि सुरक्षित छ।',
  'server.importUndoStopped': 'पूर्ववत गर्ने काम अन्त्य हुनुअघि नै रोकियो: {reason}',
  'server.importUploadOwnerOnly':
    'केवल यस आयात सुरु गर्ने व्यक्ति वा ग्लोबल एडमिनिस्ट्रेटरले मात्र यसमा फाइलहरू थप्न सक्छन्।',
  'server.importUploadNotDraft':
    'फाइलहरू आयात चल्नुअघि मात्र थप्न सकिन्छ। थप फाइलहरू थप्न, यस आयातको प्रतिलिपि बनाउनुहोस्।',
  'server.importUploadPath': 'फाइलको नाम "{path}" प्रयोग गर्न सकिँदैन: {reason}।',
  'server.importUploadTooLarge':
    'यो फाइल {limit} बाइट्सभन्दा ठुलो छ, जुन आयातले लिने सबैभन्दा ठुलो फाइल हो।',
  'server.importUploadFull':
    'यस आयातका फाइलहरूले {limit} बाइट्स नाघ्नेछन्, जुन आयातले एकै पटकमा राख्न सक्ने अधिकतम फाइल हो। केही फाइलहरू हटाउनुहोस्, वा आफ्ना फाइलहरू पुगेर सकिएपछि, पुनः प्रयास गर्नुहोस्।',
  'server.importUploadChunkTooLarge':
    'यो अनुरोधले {limit} बाइट्सभन्दा बढी बोकेको छ। फाइललाई साना टुक्राहरूमा पठाउनुहोस्।',
  'server.importUploadInProgress':
    '"{path}" अझै बाटोमै छ। यसलाई पुनः सुरु गर्नुको सट्टा, त्यसै अपलोड जारी राख्नुहोस्।',
  'server.importUploadNotFound': 'यस आयातमा "{upload}" id भएको कुनै पनि अपलोड छैन।',
  'server.importUploadOffset':
    'सर्भरले यस फाइलको {received} बाइट्स प्राप्त गरेको छ। तिनीहरू पछि आउने बाइट्सहरू पठाउनुहोस्।',
  'server.importUploadPastLength': 'यी बाइट्सहरूले यस अपलोडले घोषणा गरेको आकार नाघ्छन्।',
  'server.importUploadLength': 'फाइलको आकार बाइट्समा पठाउन Upload-Length प्रयोग गर्नुहोस्।',
  'server.importUploadMetadata':
    'फाइलको नाम "filename" को रूपमा र base64 मा यसको नाम "name" को रूपमा Upload-Metadata पठाउनुहोस्।',
  'server.importUploadOffsetMissing':
    'पहिले नै पठाइसकिएका बाइट्सको सङ्ख्याको साथ Upload-Offset पठाउनुहोस्।',
  'server.importUploadContentType':
    'Content-Type application/offset+octet-stream को साथ बाइट्स पठाउनुहोस्।',
  'server.importUploadVersion':
    'यो सर्भरले Resumable Upload प्रोटोकलको संस्करण 1.0.0 बोल्छ। Tus-Resumable: 1.0.0 पठाउनुहोस्।',
  'server.importUploadInterrupted':
    'बाइट्सहरू अनुरोध समाप्त हुनु अघि आइपुग्न छोड्यो, त्यसैले तीमध्ये कुनै पनि राखिएनन्। अफसेट माग्नुहोस् र तिनीहरूलाई फेरि पठाउनुहोस्।',
  'server.importZipEscapes': '"{name}" ZIP मा यसको फोल्डर बाहिर अवतरण हुनेछ, त्यसैले यसलाई हटाइयो।',
  'server.importZipLink': '"{name}" ZIP मा एउटा लिङ्क हो, त्यसैले यसलाई हटाइयो।',
  'server.importZipUnusable': '"{name}" ZIP मा {reason} भएको नाम छ, त्यसैले यसलाई हटाइयो।',
  'server.importZipUnsupported':
    '"{name}" ZIP मा इन्क्रिप्ट गरिएको छ वा यस्तो तरिकाले प्याक गरिएको छ जुन Romeka S ले खोल्न सक्दैन, त्यसैले यसलाई हटाइयो।',
  'server.importZipRenamed':
    '"{name}" ZIP मा ठुला अक्षरहरूलाई बेवास्ता गर्दा पहिलेको फाइलको नाम जस्तै छ, त्यसैले यसलाई "{renamed}" को रूपमा थपियो।',
  'server.importZipBomb':
    'ZIP लाई अस्वीकार गरियो: "{name}" ले आफ्नो प्याक गरिएको आकारको 100 गुणाभन्दा बढी खोल्छ। यसबाट केही पनि थपिएन।',
  'server.importZipFull':
    'ZIP लाई अस्वीकार गरियो: यसका फाइलहरूले {limit} बाइट्स पार गर्नेछन्, जुन एक आयातले राख्न सक्ने सबैभन्दा बढी हो। यसबाट केही पनि थपिएन।',
  'server.importZipTooMany':
    'ZIP लाई अस्वीकार गरियो: यसमा {count} फाइलहरू छन्, जुन 100,000 भन्दा बढी छन्। यसबाट केही पनि थपिएन।',
  'server.importZipFileTooLarge':
    'ZIP लाई अस्वीकार गरियो: "{name}" {limit} बाइट्स भन्दा ठुलो छ, जुन सबैभन्दा ठुलो फाइल हो जसले आयात लिन्छ। यसबाट केही पनि थपिएन।',
  'server.importZipUnreadable':
    'ZIP लाई अस्वीकार गरियो: यसलाई पढ्न सकिएन ({reason})। यसबाट केही पनि थपिएन।',
  'server.importZipUnpacked': 'ZIP लाई खोल्दा: {count} फाइलहरू थपिए।',
  'server.importSweepDone': '{imports} आयातहरूका फाइलहरू हटाइयो, र {uploads} अपूर्ण अपलोडहरू।',
  'server.importAccessFailed':
    'मिडिया {media} का लागि कुनै खेल्न योग्य प्रतिलिपि लाइनमा राखिएन: {reason}',
  'server.importAccessUnread':
    'यस आयातका लागि कुनै पनि खेल्न योग्य प्रतिलिपि लाइनमा राखिएन: {reason}',
  'server.mediaUploadNotFound': 'यस खातामा "{upload}" id को कुनै चरणबद्ध अपलोड छैन।',
  'server.mediaUploadBusy':
    'यो अपलोड "{upload}" अहिले थपिँदैछ। त्यसको प्रतीक्षा गर्नुहोस् र पुनः प्रयास गर्नुहोस्।',
  'server.mediaUploadIncomplete':
    'यो अपलोडमा {length} बाइट्स मध्ये {received} बाइट्स छन्। थप्नु अघि बाँकी भाग पठाउनुहोस्।',
  'server.mediaUploadIngester': 'चरणबद्ध अपलोड अपलोड इन्जेस्टरसँग थपिन्छ।',
  'server.mediaUploadInline':
    'चरणबद्ध अपलोड यसको आफ्नै मिडिया सिर्जनाको साथ थपिन्छ, वस्तु भित्र होइन।',
  'server.uploadTooLarge':
    'यो फाइल {limit} बाइट्स भन्दा ठुलो छ, जुन सबैभन्दा ठुलो फाइल हो जसले मिडिया अपलोड लिन्छ।',
  'server.mediaLinkInvalid': 'लिङ्क "{url}" http वा https ठेगाना होइन।',
  'server.mediaLinkUnreadable': 'लिङ्क "{url}" पढ्न सकिएन: {reason}',
  'server.mediaLinkStatus': 'लिङ्क "{url}" ले {status} {reason} को उत्तर दियो।',
  'server.uploadItem': 'अपलोडको वस्तु id ले अस्तित्वमा रहेको वस्तुलाई नाम दिनुपर्छ।',
  'server.mediaUploadSweepDone': '{count} चरणबद्ध अपलोडहरूले आफ्नो राखिएको समय पार गरे र हटाइयो।',
};
