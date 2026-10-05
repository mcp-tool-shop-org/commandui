<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.md">English</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

यह उन लोगों के लिए एक शेल है जिन्हें टर्मिनल से अलग करना है। CommandUI हर परिणाम को सरल शब्दों में समझाता है, आपको सरल शब्दों में कमांड पूछने देता है, और आपके द्वारा इसे देखे और स्वीकृत करने तक किसी भी तैयार कमांड को नहीं चलाता है।

## यह किसके लिए है

- जो लोग स्क्रीन रीडर का उपयोग करते हैं, या जो माउस का उपयोग नहीं करते हैं
- कम दृष्टि वाले लोग, जिन्हें बड़े पाठ या उच्च-विपरीत थीम की आवश्यकता होती है
- जो लोग टर्मिनल को समझना मुश्किल पाते हैं, जिनमें शुरुआती और संज्ञानात्मक या सीखने की अक्षमता वाले लोग शामिल हैं
- कोई भी जो कमांड चलाने से पहले उसे पढ़ना चाहता है

आपको अभी भी एक वास्तविक शेल मिलता है, जिसमें आपका अपना प्रोफ़ाइल और एक से अधिक सत्र होते हैं। कमांड टाइप करने से वह उसी तरह काम करता है जैसे पहले करता था।

## इंस्टॉल करें

- **Microsoft Store:** [Microsoft Store पर CommandUI](https://apps.microsoft.com/detail/9NTN1GFQJ91M)। स्टोर में वर्तमान में एक पुराना संस्करण है। यहां वर्णित अपडेट, इसे सबमिट करने से पहले एक्सेसिबिलिटी परीक्षण की प्रतीक्षा कर रहा है।
- **winget:** `winget install mcp-tool-shop.CommandUI` [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest) से v1.0.0 इंस्टॉल करता है।

Windows 10 या 11, x64। Ask को `qwen2.5:14b` मॉडल के साथ उसी कंप्यूटर पर [Ollama](https://ollama.com) की आवश्यकता होती है। बाकी सब कुछ इसके बिना काम करता है।

## यह क्या करता है

- **प्रत्येक परिणाम एक वाक्य में।** "समाप्त। 3 पंक्तियों का आउटपुट।" या "काम नहीं किया (निकास कोड 1)। उस कमांड में एक फ़ाइल या फ़ोल्डर मौजूद नहीं है।" विफलता **इसे ठीक करने का तरीका पूछने** और **फिर से चलाने** का विकल्प प्रदान करती है।
- **सरल शब्दों में पूछें।** कार्य का वर्णन करें, और CommandUI एक कमांड तैयार करता है, इसे समझाता है, और प्रतीक्षा करता है। **रन प्लान** अनुमोदन है, और **अस्वीकार** कुछ भी नहीं चलाता है। जब यह किसी कमांड को समझाने में असमर्थ होता है, तो यह ऐसा ही कहता है।
- **सावधानीपूर्वक हाँ।** एक कमांड जो फ़ाइलों को हटाता है या उच्च अनुमतियों की आवश्यकता होती है, तब तक प्रतीक्षा करता है जब तक आप फ़ोल्डर का नाम टाइप नहीं कर देते।
- **कमांड अभी भी आपके द्वारा टाइप किए गए को चलाता है।** यदि किसी पंक्ति में अनुरोध जैसा कुछ है, तो CommandUI वाक्य चलाने के बजाय पूछने का विकल्प प्रदान करता है।
- **आप जिन वर्कफ़्लो को बना सकते हैं।** कमांड की एक सूची बनाएं, इसे संपादित करें, इसे चलाएं और इसे हटाएं। किसी भी विलोपन को पूर्ववत किया जा सकता है। इतिहास आपके द्वारा चुने गए कमांड को सहेज सकता है।
- **इतिहास और स्मृति जिसे आप नियंत्रित करते हैं।** देखें कि क्या चला, और पढ़ें या CommandUI द्वारा नोट किए गए किसी भी चीज़ को हटाएं।

## कीबोर्ड और स्क्रीन रीडर के लिए बनाया गया

- परिणाम और त्रुटियां एक बार घोषित की जाती हैं, बिना आपके फोकस को स्थानांतरित किए।
- **आउटपुट** (Ctrl+Shift+O) प्रत्येक कमांड के आउटपुट को सरल पाठ के रूप में सूचीबद्ध करता है, प्रत्येक कमांड के लिए एक क्षेत्र, बिना किसी टर्मिनल कोड के।
- **F1** कीबोर्ड सहायता खोलता है। **Ctrl+Shift+R** अंतिम परिणाम पर जाता है। **Ctrl+Shift+A** कमांड और Ask के बीच स्विच करता है।
- प्रत्येक संवाद इसके अंदर फोकस रखता है, और Escape इसे बंद कर देता है और फोकस को उस स्थान पर वापस कर देता है जहां आप थे।
- पाठ का आकार सेटिंग्स में 100% से 200% तक बदलता है। टर्मिनल के नीचे के पैनल को छिपाया जा सकता है।
- विंडोज कंट्रास्ट थीम और कम-गति सेटिंग्स का सम्मान किया जाता है।

**अभी तक क्या परीक्षण नहीं किया गया है:** नैरेटर, NVDA और विंडोज कंट्रास्ट थीम का परीक्षण इस बिल्ड पर लोगों द्वारा नहीं किया गया है। ये परीक्षण स्टोर अपडेट से पहले किए जाएंगे। तब तक, उपरोक्त सूची को इस रूप में मानें कि ऐप को क्या करने के लिए बनाया गया है, न कि एक परीक्षण किए गए दावे के रूप में।

## सुरक्षा

CommandUI आपके मशीन पर चलता है। यह इतिहास, योजनाएं, वर्कफ़्लो, मेमोरी और सेटिंग्स को स्थानीय रूप से रखता है, और केवल उन शेल कमांड को चलाता है जिन्हें आप स्वीकृत करते हैं। यह कोई टेलीमेट्री नहीं भेजता है। Ask इस कंप्यूटर पर एक मॉडल से बात करता है। यदि वह मॉडल स्थापित नहीं है, चल नहीं रहा है, या डाउनलोड नहीं किया गया है, तो Ask ऐसा कहता है और कोई कमांड तैयार नहीं करता है।

धमकी मॉडल और भेद्यता की रिपोर्ट कैसे करें, इसके लिए [SECURITY.md](SECURITY.md) देखें।

## यह क्या नहीं है

- यह चैटबॉट नहीं है, और न ही यह अपने आप में तैयार कमांड चलाने वाली कोई चीज़ है
- यह कोई दावा नहीं है कि इस बिल्ड पर स्क्रीन रीडर या कंट्रास्ट थीम का परीक्षण किया गया है (ऊपर देखें)
- यह कंसोल नहीं है। `apps/console` इस रिपॉजिटरी में एक दूसरा फ्रंट एंड है और यह आपके द्वारा इंस्टॉल किए गए ऐप का हिस्सा नहीं है

## डेवलपर्स के लिए

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

रिलीज़ बिल्ड से स्टोर अपलोड पैक करें:

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` `release/CommandUI_<version>_x64.msix` लिखता है। यह मौजूदा स्टोर उत्पाद के पैकेज नाम, प्रकाशक और निष्पादन योग्य को रखता है, और उस संस्करण को अस्वीकार कर देता है जो अंतिम सबमिट किए गए संस्करण से अधिक नहीं है। फ़ाइल बिना हस्ताक्षर वाली है; पार्टनर सेंटर इसे साइन करता है।

```
commandui/
  apps/desktop/                 — the desktop app you install
  apps/console/                 — Rust terminal front end on the same runtime
  crates/runtime-core/          — shell sessions and events
  crates/runtime-persistence/   — local storage
  crates/runtime-planner/       — the local model Ask uses
  packages/                     — shared types, contracts, state, UI
  packaging/msix/               — Store manifest and logos
```

अधिक: [हैंडबुक](https://mcp-tool-shop-org.github.io/commandui/handbook/) · [डेवलपर सेटअप](docs/product/developer-setup.md) · [ज्ञात सीमाएं](docs/product/known-limitations.md) · [रिलीज़ चेकलिस्ट](docs/product/release-checklist.md)

## स्थिति

v1.0.2, अभी तक जारी नहीं किया गया। Microsoft Store में एक पुराना संस्करण है, और सार्वजनिक GitHub रिलीज़ v1.0.0 है।

[MCP Tool Shop](https://mcp-tool-shop.github.io/) द्वारा निर्मित।
