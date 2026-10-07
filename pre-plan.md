### Dependencies
- tracing tracy
- criterion

# Source references
Most of them should be stored locally for review and reference in /refs.
You can phyically copy as much code from there as you want. The /refs are
mainly there for you for now. I would reuse a lot of the code from there,
as possible. But since we're taking a limited feature set from each, we
can cutdown on on things we don't need.

### First rendition
- obisidian
- zeditor
- helix
- drawing: rnote
- git
- handle through source view and merge like changes for ai stuff +markup (vscode style)
- check out some of the other harness work being done here
- ai thingy: codex or something open code? or just zed
- fff
- nix stylix styling

- (might be useful? don't need to use or reference it) pdfium-render - PDF rendering using Google's PDFium library

### Possible future usuage
- matrix: element 
- anki
- obsidian graph view and links
- calibre
- Orgmode

### Requirements from sources
Zeditor:
I think we take zeditor as our raw base for the project. We'll take this as our
main environment to work off of. There's a lot of functionality here we don't
want like their login features etc. I mainly want two of the main sidebar things,
source view and the file tree. I don't need debug panel or terminal. We also don't
need lsp's, I don't use them, but language syntax highlighting would still be good.
We will probably want some of the ai stuff from this still. We don't need it to
open or jump to other projects. It should be in a sense terminal first, meaning
we're going to go to the place we want to read and manage things in a folder from
my terminal and go `editor .` and open that folder with the editor. Source view is
important, and we'll probably want to extend this functionality quite a bit in the
future as it's my main code review for ai changes currently. I tend to prefer how it
is in vscode currently than in zeditor on view/ functionality. Mainly looking through
the changes and files. 
Obsidian:
We want some of the funcitonality obsidian has, mainly for reading and editing
markdown files. The ink plugin from here would also be good. The mind map view
would be cool, I think emacs did a reimplementation that should be opensource.
But we can put off the mind map for a later feature set. The links and handling
links is something we will want to copy, I also like the rendering, and split
for things handling LaTeX maybe we can use KaTeX or w/e it's called but for
markdown (might be other formats we care about here) we want to be able to split
the view between the rendered and raw what we're editing.
Helix:
Helix is my main text editor and how I do most things. I would like all text editing
in the editor to be helix based. I like the helix keybindings for the most part. I
think zeditor uses helix already though we don't really need to support vim or anything
but we can keep it more true to helix if we need. I would like pretty much everything
in the editor to be purely navigatable by keybindings and it should fit in with the
pre existing helix ones. The existing way it's done in zeditor works fairly well.
Rnote:
I do a lot of reading and note taking and would like it all to be in one place. I did
mention the ink approach in the markdown but generally would really need something where
the editor also pretty much has rnote built in for rnote files. Basically want it such
that we can treat it as any other file but when you go to that file you're basically in
an rnote instance ready to draw and everything instead. We can consider a custom file
extension if things keep showing up in the future but keeping it interchangable with
the existing rnote should be good. In general I would like to be able to do normal editor
stuff but have one of the panes or w/e you call it as a drawing thing and have w/e else
open like text editing.

Calibre:
Calibre is mostly what I use for for pdf reading. I don't think we really want the whole
calibre db stuff in here. I mostly want to be able to have something that can read
different file formats, like viewing pdfs, epubs, etc. I believe calibre also has some
annotations stuff that might be nice to have. And for potential future things, calibre
should also do some file format conversion. But main take away and what we should focus
on for here is the pdf/ epub viewer. Along with rnote, should be able to write text,
handwrite, and read pdfs all from the same place. 

Nix:
Should use the colour pallet, fonts, and styling from my nix config. Not sure if there's
generally a good way to get them working together. But you can find my dotfiles
`~/.dotfiles` for reference. Do not change anything in there without clear instruction.
But I already use stylix for most things, and have colour schemes etc. I use everywhere.
Mainly this phd colour scheme also can mix with the green that I'm using as a background
colour. Ideally this should be configurable.

Emacs orgmode:
There's a lot of good ideas from emacs orgmode that we might want. I mentioned earlier,
that they already redid some of the obisidian stuff. But like the todo stuff, reminders,
and all of that would probably be useful functionality. I think in general our little
editor here is sort of aspiring to be closer to this where it tackles most of our main
workflow needs. This we can still fleshout a ton.

Matrix:
WE DO NOT WANT THIS RIGHT NOW THIS IS A NOTE FOR A FUTURE FEATURE. It would be nice to
be able to redo some of the zeditors social features but replace all of it with matrix
instead. Not sure if I REALLY want this at the end of the day but would consolidate
general tools I use further. Might be good to swing it in some sort of pair coding type
of way if possible. But more exploration and decision making needs to be done here.
 
# Workflows
mostly trying to consolidate my workflows into 1 editor + terminal. 
### Coding
Coding I'm mostly terminal first, run everything from the terminal. I mostly edit with
helix when I need to. Helix is sort of missing the file tree that I like from typical
editors. I usually keep a bunch of windows of tmux open to flip through projects and
handle ai sessions like claude or codex tui. I typically use so far editors like vscode
to mostly read the code and look at changes made from ai in source view. I also handle
merges by files going through ever conflict in vscode and fixing the highlighted 
### Writing
Writing I do a mix of hand writing and drawing with a tablet and text editing in markdown.
I don't have a great workflow for this yet. I mostly do this in conjunction to reading
something like a pdf. Annoying part aswell is I currently don't have a nice way to organize
my notes with what I'm reading. Having them all in one spot with this new editor should fix
that. Ideally it would also be nice to be able to do this with a textbook and be able to
read the textbook and solve problems in one place. 
### Reading
I mostly use calibre for this right now. I download stuff from annas archive etc. or arxiv.
I also use a kindle from time to time with calibre. All the books are kept together with
calibre currently, but don't have a great way to organize those on top of everything else.
I could also use zotero or something for keeping track of what I've read and references but
haven't done any of that yet and before.
