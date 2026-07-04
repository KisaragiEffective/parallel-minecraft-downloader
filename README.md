# parallel-minecraft-asset-downloader

## このツールは何？

Minecraft のアセットを並列にダウンロードします。**公式の**Minecraftランチャーは直列でしかダウンロードしないため、時間の短縮に役に立ちます。

しかし、MultiMC及びその派生のPolyMC、PrismLauncherはこの問題点を直接解決し、このツールを不要としました。そのため、このツールの更新・サポートは終了しました。

公式ランチャーはMODの文化圏と非常に相性が悪いため、これらのランチャーへ乗り換えることを**強く**推奨します。

## インストール

1. [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) をインストール
2. `cargo install --git https://github.com/KisaragiEffective/parallel-minecraft-downloader/` を実行

## 使い方

コマンドラインから、以下のコマンドを実行:

```sh
parallel-minecraft-downloader -d ~/.minecraft --version 1.12.2
```

* `-d`: `.minecraft` の場所 (参考: [minecraft.wikiの記事](https://ja.minecraft.wiki/w/.minecraft))
* `--version`: Minecraft のバージョン

## ライセンス

[Apache-2.0](https://github.com/KisaragiEffective/parallel-minecraft-downloader/blob/3a0c9c9fe43b4b17f82b35135ee74a751af8f82d/LICENSE)
