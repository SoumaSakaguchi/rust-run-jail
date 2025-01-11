# rust-run-jail

rust-run-jailは、FreeBSD上で動作するRust製のjailランタイムです。
コンフィグやテンプレートからjailの作成・実行が可能です。
実行するには管理者権限（sudo）が必要です。

## ビルド・実行方法

```
cargo b
sudo ./target/debug/rust-run-jail --help
```

## jail作成

```
sudo rust-run-jail run -p <config-path> -- <command>
```
config-path: config.jsonへのパス

command: jail内で実行するコマンド

## テンプレートjail作成

```
sudo rust-run-jail template <netns or freebsd or linux>
```
netns: ネットワークネームスペース互換

freebsd: freebsd jail

linux: linux jail

## jail削除

```
sudo rust-run-jail kill-all
```

## jail一覧表示

```
sudo rust-run-jail list
```

