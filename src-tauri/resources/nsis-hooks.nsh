; Tauri NSIS Installer Hooks — 保留用户配置（%APPDATA%）
;
; Tauri 默认卸载时会删除 %APPDATA%\PushToTalk，
; 导致用户配置（API Key、快捷键、词库等）在升级时丢失。
; 此 hook 覆盖卸载时的文件删除行为：只删安装目录，不删 AppData。

!macro customRemoveFiles
  ; 仅删除安装目录，保留 %APPDATA%\PushToTalk 中的用户配置
  RMDir /r "$INSTDIR"
!macroend