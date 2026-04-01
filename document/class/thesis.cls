% @file		thesis.cls
%
% @brief	Document class for theses.
%
% @author	Stefan Nothaas, Arbeitsgruppe Betriebssysteme, Institut für Informatik, HHU Düsseldorf
% @author Filip Krakowski, Arbeitsgruppe Betriebssyteme, Institut für Informatik, HHU Düsseldorf
%
% @since  2021/04/29 (major refactoring, minted, university logo)
% @since	2019/04/29 (bugfix, wrong page number for reference section)
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{class/thesis}[2021/04/29 Template for theses]

% Use book class as base
\LoadClass[12pt,onside,openany,a4paper]{book}


%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Packages
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\RequirePackage[left=2.5cm, right=2.5cm, top=3cm, bottom=2.5cm]{geometry}

\RequirePackage[utf8]{inputenc}
\RequirePackage[T1]{fontenc}
\RequirePackage[english,ngerman]{babel}
\RequirePackage{minted}
\RequirePackage{fancyhdr}
\RequirePackage{color}
\RequirePackage[usenames, dvipsnames]{xcolor}
\RequirePackage{graphicx}
\RequirePackage{tikz}
\RequirePackage{subcaption}
\RequirePackage{float}
\RequirePackage{multirow}
\RequirePackage{todonotes}
\RequirePackage{hyperref}
\RequirePackage{algorithm2e}
\RequirePackage{tocloft}
\RequirePackage{listings}
\RequirePackage{textcomp}
\RequirePackage{ifthen}
\RequirePackage{tcolorbox}
\RequirePackage{pgffor}
\RequirePackage{amssymb}
\RequirePackage{enumitem}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Package settings
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\tcbuselibrary{skins, breakable}
\usetikzlibrary{shadings, backgrounds}

\fancypagestyle{plain}{%
  \fancyhf{}
  \renewcommand{\headrulewidth}{0pt}
  \fancyfoot[RO,LE]{\thepage}
}

\fancypagestyle{thesis}{%
  \fancyhf{}
  \renewcommand{\headrulewidth}{0.4pt}
  \fancyfoot[RO,LE]{\thepage}
  \fancyhead[LO,RE]{\slshape \leftmark}
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Bibliography
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\usepackage[style=ieee,backend=biber]{biblatex}
\addbibresource{references.bib}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Translations
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newcommand{\translatemastersthesis}{}
\newcommand{\translatebachelorsthesis}{}
\newcommand{\translateprojectwork}{}
\newcommand{\translateprimaryreviewer}{}
\newcommand{\translatesecondaryreviewer}{}
\newcommand{\translatesupervisor}{}
\newcommand{\translatesubmittedby}{}
\newcommand{\translatefrom}{}
\newcommand{\translateuniversityname}{}
\newcommand{\translatedepartment}{}
\newcommand{\translatedeclaration}{}
\newcommand{\translatedeclarationtitle}{}
\newcommand{\translatedlistofalgorithms}{}

\addto\captionsenglish{
  \SetAlgorithmName{Algorithm}{Algorithm}{List of Algorithms}
  \renewcommand{\chaptername}{Chapter}
  \renewcommand{\translatemastersthesis}{Master's Thesis}
  \renewcommand{\translatebachelorsthesis}{Bachelor's Thesis}
  \renewcommand{\translateprojectwork}{Project Work}
  \renewcommand{\translateprimaryreviewer}{Primary Reviewer}
  \renewcommand{\translatesecondaryreviewer}{Secondary Reviewer}
  \renewcommand{\translatesupervisor}{Supervisor}
  \renewcommand{\translatesubmittedby}{submitted by}
  \renewcommand{\translatefrom}{from}
  \renewcommand{\translateuniversityname}{Heinrich Heine University Düsseldorf}
  \renewcommand{\translatedepartment}{Department Operating Systems}
  \renewcommand{\translatedlistofalgorithms}{List of Algorithms}
  \renewcommand{\translatedeclarationtitle}{Statutory Declaration}
  \renewcommand{\translatedeclaration}{I hereby state that I have written this {\thesistypename} independently and that I have not used any sources or aids other than those declared. All passages taken from the literature have been marked as such. This thesis has not yet been submitted to any examination authority in the same or a similar form.}
}

\addto\captionsngerman{
  \SetAlgorithmName{Algorithmus}{Algorithmus}{Algorithmenverzeichnis}
  \renewcommand{\chaptername}{Kapitel}
  \renewcommand{\translatemastersthesis}{Masterarbeit}
  \renewcommand{\translatebachelorsthesis}{Bachelorarbeit}
  \renewcommand{\translateprojectwork}{Projektarbeit}
  \renewcommand{\translateprimaryreviewer}{Erstgutachter}
  \renewcommand{\translatesecondaryreviewer}{Zweitgutachter}
  \renewcommand{\translatesupervisor}{Betreuer}
  \renewcommand{\translatesubmittedby}{vorgelegt von}
  \renewcommand{\translatefrom}{aus}
  \renewcommand{\translateuniversityname}{Heinrich-Heine-Universität Düsseldorf}
  \renewcommand{\translatedepartment}{Abteilung Betriebssysteme}
  \renewcommand{\translatedlistofalgorithms}{Algorithmenverzeichnis}
  \renewcommand{\translatedeclarationtitle}{Ehrenwörtliche Erklärung}
  \renewcommand{\translatedeclaration}{Hiermit versichere ich, die vorliegende {\thesistypename} selbstständig verfasst und keine anderen als die angegebenen Quellen und Hilfsmittel benutzt zu haben. Alle Stellen, die aus den Quellen entnommen wurden, sind als solche kenntlich gemacht worden. Diese Arbeit hat in gleicher oder ähnlicher Form noch keiner Prüfungsbehörde vorgelegen.}
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Variables
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newcommand{\thesistitle}[1]{\title{#1}}

\newcommand{\thesisauthor}[1]{\author{#1}}

\newcommand{\submissiondate}[1]{\date{#1}}

\def\@thesislanguage{}
\newcommand{\thesislanguage}[1]{\def\@thesislanguage{#1}}

\def\@thesiskeywords{}
\newcommand{\thesiskeywords}[1]{\def\@thesiskeywords{#1}}

\def\@authorbirthplace{}
\newcommand{\authorbirthplace}[1]{\def\@authorbirthplace{#1}}

\def\@authorbirthname{}
\newcommand{\authorbirthname}[1]{\def\@authorbirthname{#1}}

\def\@firstreviewer{}
\newcommand{\firstreviewer}[1]{\def\@firstreviewer{#1}}

\def\@secondreviewer{}
\newcommand{\secondreviewer}[1]{\def\@secondreviewer{#1}}

\def\@supervisor{}
\newcommand{\supervisor}[1]{\def\@supervisor{#1}}

\def\@thesistype{}
\newcommand{\thesistype}[1]{\def\@thesistype{#1}}

\def\@listoffiguresenabled{}
\newcommand{\listoffiguresenabled}[1]{\def\@listoffiguresenabled{#1}}

\def\@listoftablesenabled{}
\newcommand{\listoftablesenabled}[1]{\def\@listoftablesenabled{#1}}

\def\@listofalgorithmsenabled{}
\newcommand{\listofalgorithmsenabled}[1]{\def\@listofalgorithmsenabled{#1}}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Colors
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

% Primary
\definecolor{hhuCorporateBlue}{HTML}{006AB3}
\definecolor{hhuCorporateGrey}{HTML}{D9DADB}

% Secondary
\definecolor{hhuCorporateGreen}{HTML}{97BF0D}
\definecolor{hhuCorporateOrange}{HTML}{F29400}
\definecolor{hhuCorporateRed}{HTML}{BE0A26}
\definecolor{hhuCorporateDarkBlue}{HTML}{003865}
\definecolor{hhuCorporateLightBlue}{HTML}{32B8C9}

% Code
\colorlet{titlebg}{hhuCorporateGrey!80!white}
\colorlet{titlecolor}{black}
\colorlet{termbg}{hhuCorporateGrey!10!white}
\colorlet{termfg}{hhuCorporateGrey!20!white}
\colorlet{codebg}{white}
\colorlet{numberbg}{hhuCorporateGrey!25!white}
\colorlet{languagebg}{hhuCorporateGrey!20!white}
\definecolor{mintedtext}{HTML}{F8F8F2}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Document settings
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\setlength{\marginparwidth}{2.1cm}

\AtBeginDocument{

  \hypersetup{
    pdfauthor={\@author},
    pdftitle={\@title},
    pdfkeywords={\@thesiskeywords},
    pdfproducer={LaTeX with hyperref},
    pdfcreator={PDFLaTeX}
  }

  \selectlanguage{\@thesislanguage}
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Thesis environment
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newenvironment{thesis}
{

  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
  % Global options
  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

  \setlength{\parindent}{0pt}

  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
  % Disable fancy headers
  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

  \pagestyle{empty}
  \pagenumbering{gobble}

  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
  % Title page
  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

  \thesiscover
  \clearpage

  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
  % Table of contents
  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

  \tableofcontents
  \clearpage

  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
  % Re-enable fancy headers
  %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

  \pagenumbering{arabic}
  \pagestyle{thesis}
}
{
  \addcontentsline{toc}{chapter}{\bibname}
  \printbibliography
  \clearpage

  \ifthenelse{\equal{\@listoffiguresenabled}{true}}
  {%
    \addcontentsline{toc}{chapter}{\listfigurename}
    \listoffigures
    \clearpage
  }%
  {}

  \ifthenelse{\equal{\@listoftablesenabled}{true}}
  {%
    \addcontentsline{toc}{chapter}{\listtablename}
    \listoftables
    \clearpage
  }%
  {}

  \ifthenelse{\equal{\@listofalgorithmsenabled}{true}}
  {%
    \addcontentsline{toc}{chapter}{\translatedlistofalgorithms}
    \listofalgorithms
    \clearpage
  }%
  {}

  \declaration
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Title page
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newcommand{\thesiscover}
{
  \newgeometry{left=2.5cm,right=2.5cm,top=3cm}
  \begin{titlepage}

    \begin{center}

      \includegraphics[width=3cm]{img/hhu}

      \vspace{0.75cm}

      {\Large \thesistypename}

      \vspace{0.75cm}

      % Thesis title
      \vspace{4.5em}
      {\Huge \@title\\}

      \vspace{7.5em}

      {\translatesubmittedby} \\[.5em]
      {\huge \@author} \\[.5em]
      {{\translatefrom} \@authorbirthplace}\\

      \vfill


      % Department


      {\translatedepartment}\\
      Prof. Dr. Michael Schöttner\\
      {\translateuniversityname}\\

      \vspace{1cm}


      % Date
      \vspace{0.5cm}
      \@date
      \vspace{0.5cm}


      % People
      \begin{tabular}{r  l}
        {\translateprimaryreviewer}: & \@firstreviewer \\
        \ifthenelse{\equal{\@thesistype}{project}}{}{{\translatesecondaryreviewer}: & \@secondreviewer \\}
        {\translatesupervisor}: & \@supervisor \\
      \end{tabular}


    \end{center}

  \end{titlepage}
  \restoregeometry
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Literature
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newcommand{\literature}
{
  \bibliography{\literaturebibtexdatei}
  \bibliographystyle{plain}
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Statutory Declaration
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newcommand{\declaration}
{

    \thispagestyle{empty}
    \pagenumbering{gobble}

    \vspace*{6em}

    \huge
    \noindent\textbf{\translatedeclarationtitle}

    \large
    \vspace{3cm}
    {\noindent\translatedeclaration}

    \vspace{4cm}
    \begin{center}
      \begin{tabular}{lc}
        Düsseldorf, \@date \hspace{1.5cm} & \hspace{7cm} \\[-10pt]
        & \hrulefill \\
        & \@author \\
      \end{tabular}
    \end{center}
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Miscellaneous
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newcommand{\thesistypename}{%
  \ifthenelse{\equal{\@thesistype}{bachelor}}
    {\translatebachelorsthesis}
    {%
      \ifthenelse{\equal{\@thesistype}{master}}
      {\translatemastersthesis}
      {\translateprojectwork}
    }%
}

\newtcolorbox{codeblock}[3][]
{%
  enhanced,
  colbacktitle=titlebg,
  coltitle=titlecolor,
  colframe=titlebg,
  interior style={%
    top color=codebg,
    bottom color=codebg
  },
  boxrule=.5pt,
  arc=0.6mm,
  width= (\linewidth),
  fonttitle=\footnotesize,
  adjusted title=\texttt{#2},
  after title={%
    \hfill\setlength{\fboxsep}{2pt}
    \codelang{#3}
  },
  overlay={%
    \begin{tcbclipinterior}\fill[numberbg] (frame.south west)
      rectangle ([xshift=5mm]frame.north west);
    \end{tcbclipinterior}
  },
  top=0mm,
  bottom=0mm,
  right=0mm,
  left=5mm,
  #1
}

\newtcolorbox{terminalblock}
{%
  enhanced,
  colbacktitle=titlebg,
  coltitle=titlecolor,
  colframe=titlebg,
  interior style={%
    top color=codebg,
    bottom color=codebg
  },
  boxrule=.5pt,
  arc=0.6mm,
  width= (\linewidth),
  fonttitle=\footnotesize,
  adjusted title={%
    \tikz\draw[hhuCorporateGrey!50!black,fill=hhuCorporateGrey!50!black] (0,0) circle (.5ex);
    \tikz\draw[hhuCorporateGrey!60!black,fill=hhuCorporateGrey!60!black] (0,0) circle (.5ex);
    \tikz\draw[hhuCorporateGrey!70!black,fill=hhuCorporateGrey!70!black] (0,0) circle (.5ex);
  },
  top=1mm,
  bottom=0mm,
  left=1mm,
  right=1mm
}

\newtcbox{\codehighlight}{nobeforeafter,boxrule=.5pt,colback=black!5,colframe=black!35,
  arc=2pt,boxsep=0pt,left=2pt,right=2pt,top=2pt,bottom=1pt, tcbox raise base}

\newcommand{\code}[1]{\codehighlight{\texttt{#1}}}

\newtcbox{\languagehighlight}{nobeforeafter,boxrule=0pt,colback=black!5,arc=1pt,
  boxsep=0pt,left=2pt,right=2pt,top=2pt,bottom=2pt, tcbox raise base}

\newcommand{\codelang}[1]{\languagehighlight{\texttt{#1}}}

\newenvironment{noteblock}%
{
  \setlength\topsep{4pt}
  \setlength\parskip{0pt}
  \begin{center}
    \begin{tabular}{|p{.95\linewidth}}
    }
    %
    {
    \end{tabular}
  \end{center}
}

%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
% Code environments
%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%

\newminted{java}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=0.8,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontfamily=tt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{kotlin}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=0.8,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontfamily=tt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{scala}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=0.8,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontfamily=tt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{c}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=0.8,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontfamily=tt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{cpp}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=0.8,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontfamily=tt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{rust}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=1.0,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{go}{
  linenos,
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=1.0,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}

\newminted{text}{
  autogobble,
  breaklines=true,
  breaksymbol=,
  breakindent=4mm,
  breakbefore=.,
  breakbeforesymbolpre=,
  breakbeforesymbolpost=,
  baselinestretch=1.0,
  escapeinside=||,
  framesep=5mm,
  numbersep=7pt,
  fontsize=\footnotesize,
  style=bw,
  escapeinside=||
}


