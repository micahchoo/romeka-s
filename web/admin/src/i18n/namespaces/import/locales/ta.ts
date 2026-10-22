/**
 * The Tamil Import page messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `ImportMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ImportMessages } from "../index";

export const ta: ImportMessages = {
  "home.lede":
    "Omeka-வில் கோப்புகளின் கோப்புறையை உருப்படிகளாகச் சேர்க்கவும். கடைசிப் படி வரை Omeka-வில் எதுவும் சேராது, பிறகு நீங்கள் இறக்குமதியை ரத்து செய்யவும் முடியும்.",
  "home.where": "உங்கள் கோப்புகள் எங்கே உள்ளன?",
  "home.server.title": "சர்வரில் உள்ள ஒரு கோப்புறையில்",
  "home.server.note":
    "உங்கள் Omeka-வை இயக்கும் நபர் அங்கே வைத்த கோப்புறைகளுக்கு.",
  "home.server.none":
    "இறக்குமதிக்காக சர்வரில் எந்தக் கோப்புறையும் அமைக்கப்படவில்லை. உங்கள் Omeka-வை இயக்கும் நபர் ROMEKA_ARCHIVE_ROOTS மூலம் ஒன்றை அமைக்கலாம்.",
  "home.server.roots": "சர்வரில் உள்ள கோப்புறைகள்",
  "home.server.loading": "கோப்புறையைப் படித்தல்...",
  "home.server.path": "கோப்புறை",
  "home.server.files":
    "இந்தக் கோப்புறையில் {count} கோப்புகள் உள்ளன; அதற்குள் உள்ள கோப்புறைகள் கணக்கில் சேரவில்லை.",
  "home.server.filesOne":
    "இந்தக் கோப்புறையில் 1 கோப்பு உள்ளது; அதற்குள் உள்ள கோப்புறைகள் கணக்கில் சேரவில்லை.",
  "home.server.noFolders": "இதற்குள் கோப்புறைகள் எதுவும் இல்லை.",
  "home.server.folders": "கோப்புறைகள்",
  "home.server.use": "{folder} கோப்புறையைப் பயன்படுத்தவும்",
  "home.server.useAll": "{root} இல் உள்ள அனைத்தையும் பயன்படுத்தவும்",
  "home.server.starting": "தொடங்குதல்...",
  "home.server.top": "மேல் கோப்புறைக்குச் செல்லவும்",
  "home.computer.title": "இந்த கணினியில்",
  "home.computer.note":
    "இந்த கணினியிலிருந்து கோப்புகள், ஒரு கோப்புறை அல்லது ஒரு ஜிப் கோப்பை அனுப்பவும். நீங்கள் அவற்றை அடுத்த பக்கத்தில் சேர்க்கிறீர்கள்.",
  "home.computer.start": "இந்த கணினியிலிருந்து கோப்புகளுடன் தொடங்கவும்",
  "home.past.title": "கடந்த இறக்குமதிகள்",
  "home.past.empty":
    "கடந்த இறக்குமதிகள் இன்னும் இல்லை. ஒன்றைத் தொடங்க மேலே உள்ள கோப்புறையைத் தேர்ந்தெடுக்கவும்.",
  "home.past.resource": "இறக்குமதிகள்",
  "home.past.colFiles": "கோப்புகள்",
  "home.past.colStatus": "நிலை",
  "home.past.colStarted": "தொடங்கியது",
  "home.past.colFinished": "முடிந்தது",
  "home.past.colAdded": "சேர்க்கப்பட்டது",
  "home.past.colMore": "மேலும்",
  "home.past.added": "{total} இல் {done}",
  "home.past.copy": "நகலை உருவாக்கு",
  "home.past.copying": "நகலெடுக்கப்படுகிறது...",
  "home.past.retry": "மீண்டும் முயற்சிக்கவும்",
  "words.folder": "{path} ({root})",
  "words.root": "{root} முழுவதும்",
  "words.computer": "இந்த கணினியிலிருந்து கோப்புகள்",
  "words.computerLists": "இந்த கணினியிலிருந்து பட்டியல்கள்",
  "words.none": "இன்னும் கோப்புகள் இல்லை",
  "status.draft": "இன்னும் சேர்க்கப்படவில்லை",
  "status.running": "சேர்க்கப்படுகிறது",
  "status.stopping": "நிறுத்தப்படுகிறது",
  "status.done": "சேர்க்கப்பட்டது",
  "status.run_failed": "இறுதிக்கு முன்பே நிறுத்தப்பட்டது",
  "status.undoing": "ரத்து செய்யப்படுகிறது",
  "status.undone": "ரத்து செய்யப்பட்டது",
  "status.partly_undone":
    "பகுதியளவு ரத்து செய்யப்பட்டது: சில உருப்படிகள் வைக்கப்பட்டன",
  "status.undo_failed": "இறுதிக்கு முன்பே ரத்து நிறுத்தப்பட்டது",
  "shell.loading": "இறக்குமதியை ஏற்றுகிறது...",
  "shell.firstLook": "உங்கள் கோப்புகளை முதல் முறையாக ஆய்வு செய்கிறது...",
  "shell.badId": '"{param}" என்பது ஒரு இறக்குமதியின் எண் அல்ல.',
  "shell.badScreen": "இந்த இறக்குமதியில் அந்தப் பெயரில் ஒரு பக்கம் இல்லை.",
  "shell.fromStart": "இறக்குமதியைத் தொடக்கத்திலிருந்து திறக்கவும்",
  "shell.home": "அனைத்து இறக்குமதிகள்",
  "shell.mode": "முறை",
  "shell.simple": "எளிய முறை",
  "shell.advanced": "மேம்பட்ட முறை",
  "shell.simpleNote":
    "ஒரு நேரத்தில் ஒரு கேள்வி, பரிந்துரைக்கப்பட்ட பதில்களுடன்.",
  "shell.advancedNote":
    "அனைத்து அமைப்புகளும் ஒரே நேரத்தில். எளிய முறையில் உள்ள உங்கள் பதில்களும் இதே அமைப்புகள்தான்.",
  "shell.steps": "படிகள்",
  "shell.settings": "அமைப்புகள்",
  "shell.back": "பின்செல்",
  "shell.next": "அடுத்தது",
  "shell.saved": "சேமிக்கப்பட்டது",
  "shell.saving": "சேமிக்கப்படுகிறது...",
  "shell.notSaved": "சேமிக்கப்படவில்லை",
  "shell.stale":
    "நீங்கள் இந்த இறக்குமதியைத் திறந்த பிறகு, ஒருவேளை வேறொரு தாவலில், இது வேறு இடத்தில் சேமிக்கப்பட்டது. அதற்குப் பிறகு நீங்கள் செய்த மாற்றங்கள் சேமிக்கப்படவில்லை.",
  "shell.staleLoad": "சேமிக்கப்பட்ட பதில்களை ஏற்றவும்",
  "shell.staleNote":
    "இது அதற்குப் பிறகு நீங்கள் செய்த மாற்றங்களை நீக்கிவிடும்.",
  "shell.failed": "உங்கள் கடைசி மாற்றம் சேமிக்கப்படவில்லை.",
  "shell.retry": "மீண்டும் முயற்சிக்கவும்",
  "shell.closed":
    "இந்த இறக்குமதி தொடங்கப்பட்டுவிட்டது, எனவே அதன் பதில்களை மாற்ற முடியாது. அவற்றை மாற்ற, ஒரு நகலை உருவாக்கவும்.",
  "shell.copy": "நகலை உருவாக்கவும்",
  "shell.delete": "இந்த இறக்குமதியை நீக்கவும்",
  "line.items": "இதுவரை {count} உருப்படிகள்.",
  "line.itemsOne": "இதுவரை 1 உருப்படி.",
  "line.left": "{count} விடப்பட்டன.",
  "line.look": "{count} வகையான சிக்கல்களை ஆய்வு செய்ய",
  "line.lookOne": "1 வகையான சிக்கலை ஆய்வு செய்ய",
  "line.nothing": "எதற்கும் ஆய்வு தேவையில்லை.",
  "line.stops": "சேர்ப்பதற்கு முன் {count} சரிசெய்யப்பட வேண்டும்.",
  "line.notYet": "நீங்கள் சேர்க்கும் வரை Omeka-வில் எதுவும் இருக்காது.",
  "line.unread": "எண்ணிக்கையைப் படிக்க முடியவில்லை: {message}",
  "advanced.line":
    "{items} உருப்படிகள் · {left} விடப்பட்டன · {changes} மாற்றங்கள் · செயல் {action}",
  "simple.files.title": "உங்கள் கோப்புகள் எங்கே உள்ளன?",
  "simple.structure.title":
    "உங்கள் காப்பகம் எவ்வாறு உருப்படிகள் மற்றும் தொகுப்புகளாக மாறுகிறது",
  "simple.structure.step": "கட்டமைப்பு",
  "simple.found.title": "உங்கள் கோப்புகளில் கண்டவை",
  "simple.example.title": "ஒரு உதாரணத்தைக் காட்டு",
  "simple.allFiles.title": "ஒவ்வொரு கோப்பும்",
  "simple.list.title": "அவற்றை விவரிக்கும் ஒரு பட்டியல் உங்களிடம் உள்ளதா?",
  "simple.pair.title": "வரிசைகளையும் கோப்புகளையும் கையால் இணைக்கவும்",
  "simple.where.title": "அவை Omeka-வில் எங்கே செல்ல வேண்டும்?",
  "simple.check.title": "சரிபார்க்கவும்",
  "simple.check.step": "சரிபார்க்கவும்",
  "simple.allItems.title": "ஒவ்வொரு உருப்படியும்",
  "simple.add.title": "அவற்றைச் சேர்க்கவும்",
  "simple.add.step": "சேர்க்கவும்",
  "advanced.files.title": "ஆதாரங்கள், முறை மற்றும் கோப்புகள்",
  "advanced.files.step": "கோப்புகள்",
  "advanced.lists.title":
    "பட்டியல்கள்: வாசித்தல், இணைத்தல் மற்றும் நெடுவரிசைகள்",
  "advanced.lists.step": "பட்டியல்கள்",
  "advanced.fields.title": "இயல்புநிலை மற்றும் புல விதிகள்",
  "advanced.fields.step": "புலங்கள்",
  "advanced.problems.title": "ஒவ்வொரு வகையான சிக்கலுடனும் என்ன செய்ய வேண்டும்",
  "advanced.problems.step": "சிக்கல்கள்",
  "advanced.existing.title": "செயல் மற்றும் பொருத்தம்",
  "advanced.existing.step": "ஏற்கனவே உள்ள உருப்படிகள்",
  "advanced.changes.title": "நீங்கள் செய்த ஒவ்வொரு மாற்றமும்",
  "advanced.changes.step": "மாற்றங்கள்",
  "advanced.items.title": "ஒவ்வொரு உருப்படியும்",
  "advanced.items.step": "உருப்படிகள்",
  "advanced.run.title": "இறக்குமதியை இயக்கவும்",
  "advanced.run.step": "இயக்கவும்",
  "advanced.allFiles.title": "ஒவ்வொரு கோப்பும்",
  "advanced.pair.title": "வரிசைகளையும் கோப்புகளையும் கையால் இணைக்கவும்",
  "upload.heading": "இந்த கணினியிலிருந்து கோப்புகளைச் சேர்க்கவும்",
  "upload.dropHint":
    "கோப்புகள், ஒரு கோப்புறை அல்லது ஒரு ஜிப் கோப்பை இங்கே இழுக்கவும்.",
  "upload.dropNow": "அவற்றைச் சேர்க்க கீழே விடவும்.",
  "upload.chooseFiles": "கோப்புகளைத் தேர்ந்தெடுக்கவும்",
  "upload.chooseFolder": "ஒரு கோப்புறையைத் தேர்ந்தெடுக்கவும்",
  "upload.zipNote":
    "ஒரு ஜிப் கோப்பு வந்தவுடன் அது விரிவுபடுத்தப்படும். புள்ளி (dot) கொண்டு தொடங்கும் பெயர்களைக் கொண்ட கோப்புகள் மற்றும் கோப்புறைகள் தவிர்க்கப்படும்.",
  "upload.limits":
    "ஒரு கோப்பு {file} வரை இருக்கலாம். இந்த இறக்குமதியின் அனைத்து கோப்புகளும் சேர்ந்து {total} வரை இருக்கலாம். தற்போது பயன்பாட்டில்: {used}.",
  "upload.reading": "நீங்கள் எதை வைத்தீர்களோ அதை வாசிக்கிறது...",
  "upload.loading": "ஏற்கனவே சேர்க்கப்பட்ட கோப்புகளை வாசிக்கிறது...",
  "upload.stored":
    "இந்த கணினியிலிருந்து நீங்கள் சேர்த்த கோப்புகள்: {count}, {size}.",
  "upload.skipped":
    "புள்ளி (dot) கொண்டு தொடங்கும் பெயரால் தவிர்க்கப்பட்டது: {count}.",
  "upload.alreadyHere":
    "ஏற்கனவே சேர்க்கப்பட்டது, எனவே மீண்டும் அனுப்பப்படாது: {count}.",
  "upload.overallLabel": "அனைத்து கோப்புகள்",
  "upload.overall":
    "{size} இல் {sent} அனுப்பப்பட்டது. முடிந்த கோப்புகள்: {count} இல் {done}.",
  "upload.pauseAll": "அனைத்து செயல்பாடுகளையும் நிறுத்து",
  "upload.resumeAll": "அனைத்து செயல்பாடுகளையும் தொடங்கு",
  "upload.leaveNote":
    "நீங்கள் இந்த பக்கத்தை விட்டு வெளியேறினால், அனுப்புதல் நிறுத்தப்படும். மீண்டும் வந்து அதே கோப்புகளைச் சேர்த்து, தொடரவும்.",
  "upload.listLabel": "அனுப்பப்படும் கோப்புகள்",
  "upload.doneList": "அனுப்பப்பட்டது: {count}",
  "upload.moreRows": "மேலும் {count} உள்ளன.",
  "upload.progress": "{size} இல் {sent}",
  "upload.state.waiting": "காத்திருக்கிறது",
  "upload.state.sending": "அனுப்புதல்",
  "upload.state.paused": "நிறுத்தி வைக்கப்பட்டது",
  "upload.state.needsFile":
    "{size} இல் {sent} இடத்தில் நிறுத்தப்பட்டது. தொடர இந்த கோப்பை மீண்டும் சேர்க்கவும்.",
  "upload.state.unpacking": "ஜிப் கோப்பை விரிவுபடுத்துதல்...",
  "upload.state.done": "அனுப்பப்பட்டது",
  "upload.state.unpacked":
    "விரிவுபடுத்தப்பட்டது. அதன் கோப்புகள் சேர்க்கப்பட்டன.",
  "upload.pause": "இடைநிறுத்து",
  "upload.resume": "தொடர",
  "upload.remove": "நீக்கு",
  "upload.retry": "மீண்டும் முயற்சிக்கவும்",
  "upload.pauseFile": "{name} ஐ நிறுத்தி வைக்கவும்",
  "upload.resumeFile": "{name} ஐத் தொடரவும்",
  "upload.removeFile": "{name} ஐ நீக்கவும்",
  "upload.retryFile": "மீண்டும் {name} ஐ முயற்சிக்கவும்",
  "upload.announce.finished":
    "{name} அனுப்பப்பட்டது. முடிந்த கோப்புகள்: {count} இல் {done}.",
  "upload.announce.failed": "{name} அனுப்பப்படவில்லை. {reason}",
  "upload.announce.allDone": "அனைத்து கோப்புகளும் அனுப்பப்பட்டன.",
  "upload.problem.fileTooLarge":
    "இந்த கோப்பு {limit}-ஐ விட பெரியது, ஒரு கோப்பிற்கு இருக்கக்கூடிய அதிகபட்ச அளவு இதுவே. இதைச் சிறிய கோப்புகளாகப் பிரிக்கவும், அல்லது வரம்பை உயர்த்த நிர்வாகியிடம் கேட்கவும்.",
  "upload.problem.importFull":
    "இந்த இறக்குமதியில் {limit} இடமில்லை, இது {used} இடத்தைப் பயன்படுத்துகிறது. சில கோப்புகளை நீக்கவும், அல்லது மீதமுள்ளவற்றை மற்றொரு இறக்குமதியில் வைக்கவும்.",
  "upload.problem.badName":
    "சேவையகத்தால் இந்த பெயரைப் பயன்படுத்த முடியாது. கோப்பு அல்லது கோப்புறையின் பெயரை மாற்றவும், பின்னர் அதை மீண்டும் சேர்க்கவும்.",
  "upload.problem.importStarted":
    "இந்த இறக்குமதி தொடங்கப்பட்டுவிட்டது, எனவே இனி கோப்புகளைச் சேர்க்க முடியாது. இதை மீண்டும் சேர்க்க, ஒரு நகலை உருவாக்கவும்.",
  "upload.problem.notAllowed":
    "இந்த இறக்குமதியைத் தொடங்கிய நபர் அல்லது ஒரு நிர்வாகி மட்டுமே இதில் கோப்புகளைச் சேர்க்க முடியும்.",
  "upload.problem.gone":
    "சேவையகத்தில் ஏற்கனவே அனுப்பப்பட்ட பகுதி இனி இல்லை. ஒரு நாள் கழித்து முடிக்கப்படாத கோப்புகள் நீக்கப்படும். மீண்டும் அனுப்ப, தொடக்கத்திலிருந்து முயற்சிக்கவும்.",
  "upload.problem.zipRefused":
    "ஜிப் கோப்பு விரிவுபடுத்தப்படவில்லை: {reason} ஜிப் கோப்பைச் சரிசெய்து, அதை நீக்கிவிட்டு, மீண்டும் சேர்க்கவும்.",
  "upload.problem.connection":
    "இணைப்பு துண்டிக்கப்பட்டது. அதைச் சரிபார்த்து, மீண்டும் முயற்சிக்கவும்.",
  "upload.problem.server": "சேவையகம் இந்த கோப்பை ஏற்கவில்லை: {message}",
  "paged.loading": "பக்கம் {page} ஏற்றப்படுகிறது...",
  "paged.search": "தேடல்",
  "paged.showAll": "அனைத்தையும் காட்டு",
  "problem.empty-required": "மதிப்பு தேவைப்படும் ஒரு புலம் காலியாக உள்ளது",
  "problem.date-form": "Omeka வரிசைப்படுத்தக்கூடிய வடிவில் இல்லாத ஒரு தேதி",
  "problem.space-at-ends":
    "ஒரு மதிப்பின் தொடக்கத்திலோ அல்லது இறுதியிலோ உள்ள இடைவெளிகள்",
  "problem.encoding-suspect":
    "உங்கள் பட்டியலில் தவறாக வாசிக்கப்பட வாய்ப்புள்ள எழுத்துக்கள்",
  "problem.name-encoding": "தவறாக வாசிக்கப்பட வாய்ப்புள்ள கோப்பு பெயர்",
  "problem.cells-count": "தவறான எண்ணிக்கையிலான செல்களைக் கொண்ட ஒரு வரிசை",
  "problem.no-file":
    "இந்த இறக்குமதியின் எந்தக் கோப்பின் பெயரையும் குறிப்பிடாத வரிசை",
  "problem.not-media": "படம், ஒலி, திரைப்படம் அல்லது PDF அல்லாத ஒன்று",
  "problem.listed-not-media": "Omeka ஏற்காத உங்கள் பட்டியலில் உள்ள கோப்பு",
  "problem.format-mismatch": "பெயரின் முடிவு உள்ளடக்கத்துடன் பொருந்தாத கோப்பு",
  "problem.exists": "ஏற்கனவே Omeka-வில் உள்ளது",
  "problem.unmatched": "Omeka-வில் மாற்ற வேண்டிய உருப்படி எதுவும் இல்லை",
  "problem.many-matches":
    "Omeka-வில் ஒன்றுக்கும் மேற்பட்ட உருப்படிகள் பொருந்துகின்றன",
  "problem.many-values":
    "Omeka ஒரு மதிப்பை மட்டுமே சேமிக்கும் இடத்தில் ஒன்றுக்கும் மேற்பட்ட மதிப்புகள்",
  "problem.link-missing": "இல்லாத ஒரு உருப்படிக்கான இணைப்பு",
  "problem.unknown-type": "Omeka-வில் இல்லாத ஒரு உருப்படி வகை",
  "problem.unknown-set": "அங்கு இல்லாத ஒரு உருப்படித் தொகுப்பு",
  "problem.unknown-kind":
    "ஒரு உருப்படியாகவோ, ஊடகமாகவோ அல்லது உருப்படித் தொகுப்பாகவோ இல்லாத ஒரு வரிசை",
  "problem.duplicate-row": "ஒரு உருப்படிக்கான இரண்டு வரிசைகள்",
  "problem.unknown-term": "Omeka-வில் இல்லாத ஒரு புலத்திற்கான நெடுவரிசை",
  "problem.template-missing": "Omeka-வில் இனி இல்லாத ஒரு உருப்படி வகை",
  "problem.type-exists":
    "Omeka-வில் உள்ள ஒரு உருப்படியின் பெயரைக் கொண்ட புதிய உருப்படி வகை",
  "problem.encoding-unsupported":
    "Omeka-வால் படிக்க முடியாத வகை உரையில் உள்ள பட்டியல்",
  "tag.kind.exif": "கேமரா தரவு (EXIF)",
  "tag.kind.iptc": "பத்திரிகைத் தரவு (IPTC)",
  "tag.kind.xmp": "திருத்தத் தரவு (XMP)",
  "tag.kind.file": "கோப்பைப் பற்றி",
  "tag.DateTimeOriginal": "எடுக்கப்பட்ட தேதி",
  "tag.Artist": "புகைப்படக் கலைஞர் அல்லது கலைஞர்",
  "tag.Copyright": "பதிப்புரிமை",
  "tag.ImageDescription": "பட விளக்கம்",
  "tag.GPS": "ஜி.பி.எஸ். நிலை",
  "tag.Model": "கேமரா மாதிரி",
  "tag.IPTC By-line": "உருவாக்கியவர் (IPTC)",
  "tag.IPTC Caption-Abstract": "படக் குறிப்பு (IPTC)",
  "tag.IPTC Keywords": "முக்கியச் சொற்கள் (IPTC)",
  "tag.IPTC Object Name": "தலைப்பு (IPTC)",
  "tag.IPTC Date Created": "உருவாக்கப்பட்ட தேதி (IPTC)",
  "tag.IPTC City": "நகரம் (IPTC)",
  "tag.IPTC Country": "நாடு (IPTC)",
  "tag.XMP dc:title": "தலைப்பு (XMP)",
  "tag.XMP dc:description": "விளக்கம் (XMP)",
  "tag.XMP dc:creator": "உருவாக்கியவர் (XMP)",
  "tag.XMP dc:subject": "பொருண்மைகள் (XMP)",
  "tag.XMP dc:rights": "உரிமைகள் (XMP)",
  "tag.XMP photoshop:DateCreated": "உருவாக்கப்பட்ட தேதி (XMP)",
  "tag.XMP lr:hierarchicalSubject": "முக்கியச் சொல் மரம் (XMP)",
  "tag.File name": "கோப்புப் பெயர்",
  "tag.Folder": "கோப்புறை",
  "tag.Size": "கோப்பு அளவு",
  "tag.Pixel dimensions": "பிக்சல்களில் அளவு",
  "tag.Format": "கோப்பு வடிவம்",
  "trip.again": "இந்தப் பட்டியலை மீண்டும் பதிவேற்றவும்",
  "trip.againNext":
    "நீங்கள் அனுப்பும் அடுத்த கோப்பு {file}-இன் இடத்தைப் பிடிக்கும்.",
  "trip.lacking":
    "அனுப்பப்படவில்லை: {file}-இல் “{column}” என்ற நெடுவரிசை இல்லை, அதை {count} பதில்கள் பயன்படுத்துகின்றன.",
  "trip.lackingOne":
    "அனுப்பப்படவில்லை: {file}-இல் “{column}” என்ற நெடுவரிசை இல்லை, அதை 1 பதில் பயன்படுத்துகிறது.",
};
