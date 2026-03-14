<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.md">English</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# कमांडयूआई

एआई-आधारित शेल वातावरण, जिसमें सिमेंटिक कमांड समीक्षा की सुविधा है।

## यह क्या करता है

- वास्तविक पीटीवाई शेल सत्र (यह कोई रैपर या चैटबॉट नहीं है)
- दो इनपुट विकल्प: सीधा टर्मिनल इनपुट (स्वतंत्र) + कंपोजर (संरचित/ट्रैक किया गया)
- सिमेंटिक मोड: इरादे का वर्णन करें → एआई कमांड उत्पन्न करता है → आप समीक्षा/संपादित/अनुमोदन करते हैं
- जोखिम स्तर के अनुसार पुष्टि: निम्न (स्वचालित), मध्यम (कॉन्फ़िगर करने योग्य), उच्च (आवश्यक)
- इतिहास, जिसमें पुनः चलाने, योजना को फिर से खोलने और वर्कफ़्लो में सहेजने जैसे विकल्प हैं।
- सहेजे गए वर्कफ़्लो: किसी भी कमांड को पुन: प्रयोज्य वर्कफ़्लो में बदलें।
- प्रोजेक्ट-आधारित मेमोरी: बार-बार किए गए संपादन से प्राथमिकताओं को सीखता है।
- मल्टी-सेशन टैब, जिसमें प्रत्येक सत्र के लिए टर्मिनल स्ट्रीम हैं।
- स्थानीय SQLite डेटाबेस (इतिहास, योजनाएं, वर्कफ़्लो, मेमोरी, सेटिंग्स)
- क्लासिक बनाम गाइडेड मोड, जिनमें वास्तविक व्यवहार संबंधी अंतर हैं।

## यह क्या नहीं है

- यह कोई चैटबॉट या स्वायत्त एजेंट नहीं है।
- यह टर्मिनल एमुलेटर का विकल्प नहीं है।
- यह उत्पादन के लिए पूरी तरह से तैयार नहीं है (प्रारंभिक संस्करण 0)

## कार्यक्षेत्र का लेआउट

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## शुरुआत कैसे करें

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## दस्तावेज़

- [डेवलपर सेटअप](docs/product/developer-setup.md)
- [ज्ञात सीमाएं](docs/product/known-limitations.md)
- [स्मोक टेस्ट चेकलिस्ट](docs/specs/smoke-test-checklist.md)
- [रिलीज़ चेकलिस्ट](docs/product/release-checklist.md)

## वर्तमान स्थिति

प्रारंभिक संस्करण 0, जिसमें एक वास्तविक शेल कोर है। 21 घटकों का बूटस्ट्रैप, जिसमें शामिल हैं: पीटीवाई सत्र, सिमेंटिक समीक्षा लूप, डेटाबेस, मेमोरी, वर्कफ़्लो, पहुंच सेटिंग्स, मल्टी-सेशन टैब, xterm.js टर्मिनल, प्रॉम्प्ट-मार्कर पूर्णता का पता लगाना।
