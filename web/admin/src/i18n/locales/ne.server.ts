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
  'server.siteItemsOneSelector': '"query", "items" वा "item_sets" मध्ये ठिक एक',
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
};
