Require curl
Output demo.gif
Set Theme "Catppuccin Mocha"
Set Margin 20
Set MarginFill "#b4befe"
Set FontSize 20
Set Width 1200
Set Height 600
Set BorderRadius 15
Set WindowBar Colorful
Set Framerate 60
Set Padding 20
Set Shell fish

Sleep 500ms
Type@30ms "curl -fsSL https://raw.githubusercontent.com/ImShyMike/timesplit/refs/heads/main/install.sh | sudo bash -s -- update"
Enter

Type@150ms "REDACTED"
Hide
Enter
Show

Wait+Line

Sleep 1s
Type@50ms "timesplit setup"
Enter

Wait+Line

Sleep 5s
