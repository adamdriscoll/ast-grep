from ast_grep_py import SgRoot

source = """
Get-ChildItem $HOME | ForEach-Object { $_.Name }
Write-Host $name
""".strip()
root = SgRoot(source, "powershell").root()

def test_powershell_aliases():
    for alias in ["powershell", "pwsh", "ps1"]:
        assert SgRoot(source, alias).root().find(pattern="Write-Host $A")

def test_powershell_capture():
    node = root.find(pattern="Write-Host $A")
    assert node
    assert node.get_match("A").text() == "$name"

def test_powershell_literal_variables():
    assert root.find(pattern="$_.Name")
    assert root.find(pattern="Get-ChildItem $Home")
    assert root.find(pattern="Get-ChildItem $profile") is None

def test_powershell_multi_capture():
    node = root.find(pattern="Get-ChildItem $DIR | ForEach-Object { $$$BODY }")
    assert node
    assert [n.text() for n in node.get_multiple_matches("BODY")] == ["$_.Name"]

def test_powershell_fix():
    node = root.find(pattern="Write-Host $A")
    assert node
    edit = node.replace("Write-Output $name")
    assert root.commit_edits([edit]).endswith("Write-Output $name")
