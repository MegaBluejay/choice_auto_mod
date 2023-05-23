use anyhow::Result;
use clap::Parser;
use indoc::indoc;
use smali::{
    parse_fragment,
    types::{
        MethodSignature, Modifier, SmaliClass,
        SmaliInstruction::{Instruction, Label},
        SmaliMethod, TypeSignature,
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[macro_use]
extern crate simple_error;

#[derive(Parser, Debug)]
struct Args {
    project_path: PathBuf,
}

const ADD_MOD_BUTTON: &str = indoc! {r#"
    const/16 v0, 0xc
    const-string v1, "Mod"
    invoke-direct {p0, p1, v0, v1}, Lcom/choiceofgames/choicescript/ChoiceScriptActivity;->addMenu(Landroid/view/Menu;ILjava/lang/String;)Landroid/view/MenuItem;
    const/4 v0, 0x1
    return v0
"#};

const ON_MOD_PRESSED: &str = indoc! {r#"
    :pswitch_mod
    invoke-direct {p0}, Lcom/choiceofgames/choicescript/ChoiceScriptActivity;->startMod()V
    const/4 v0, 0x1
    return v0
"#};

const START_MOD: &str = indoc! {r#"
    const/4 v0, 0x1
    iput-boolean v0, p0, Lcom/choiceofgames/choicescript/ChoiceScriptActivity;->statsMode:Z
    invoke-virtual {p0}, Lcom/choiceofgames/choicescript/ChoiceScriptActivity;->getSupportActionBar()Landroidx/appcompat/app/ActionBar;
    move-result-object v1
    if-eqz v1, :cond_0
    const-string v2, "Mod"
    invoke-virtual {v1, v2}, Landroidx/appcompat/app/ActionBar;->setTitle(Ljava/lang/CharSequence;)V
    invoke-virtual {v1, v0}, Landroidx/appcompat/app/ActionBar;->setDisplayHomeAsUpEnabled(Z)V
    invoke-virtual {v1, v0}, Landroidx/appcompat/app/ActionBar;->setDisplayShowTitleEnabled(Z)V
    invoke-virtual {p0}, Lcom/choiceofgames/choicescript/ChoiceScriptActivity;->supportInvalidateOptionsMenu()V

    :cond_0
    const-string v0, "showMod"
    const/4 v1, 0x0
    new-array v2, v1, [Ljava/lang/String;
    invoke-virtual {p0, v0, v2}, Lcom/choiceofgames/choicescript/ChoiceScriptActivity;->callback(Ljava/lang/String;[Ljava/lang/String;)V

    return-void
"#};

const RETURN_TRUE: &str = indoc! {r#"
    const/4 v0, 0x1
    return v0
"#};

fn patch_class<F>(class_path: &Path, patch: F) -> Result<()>
where
    F: Fn(&mut SmaliClass) -> Result<()>,
{
    let mut class = SmaliClass::read_from_file(class_path)?;
    patch(&mut class)?;
    class.write_to_file(class_path)?;
    Ok(())
}

fn patch_main_activity(main_activity: &mut SmaliClass) -> Result<()> {
    let options_menu = require_with!(
        main_activity
            .methods
            .iter_mut()
            .find(|m| m.name == "onCreateOptionsMenu"),
        "couldn't find onCreateOptionsMenu"
    );
    options_menu.instructions.pop();
    options_menu
        .instructions
        .extend(parse_fragment(ADD_MOD_BUTTON)?);

    let item_selected = require_with!(
        main_activity
            .methods
            .iter_mut()
            .find(|m| m.name == "onOptionsItemSelected"),
        "couldn't find onOptionsItemSelected"
    );
    let ret_pos = require_with!(
        item_selected.instructions.iter().position(|i| match i {
            Instruction(s) => s.starts_with("return"),
            _ => false,
        }),
        "couldn't find return in onOptionsItemSelected"
    );
    item_selected
        .instructions
        .splice(ret_pos + 1..ret_pos + 1, parse_fragment(ON_MOD_PRESSED)?);

    let switch_end = require_with!(
        item_selected.instructions.iter().position(|i| match i {
            Instruction(s) => s == ".end packed-switch",
            _ => false,
        }),
        "couldn't find pswitch end in onOptionsItemSelected"
    );
    item_selected
        .instructions
        .insert(switch_end, Label("pswitch_mod".to_string()));

    let start_mod = SmaliMethod {
        name: "startMod".to_string(),
        modifiers: vec![Modifier::Private],
        constructor: false,
        signature: MethodSignature {
            args: vec![],
            return_type: TypeSignature::Void,
        },
        locals: 3,
        annotations: vec![],
        instructions: parse_fragment(START_MOD)?,
    };
    main_activity.methods.push(start_mod);
    Ok(())
}

fn patch_billing(billing: &mut SmaliClass) -> Result<()> {
    let already_purchased = require_with!(
        billing
            .methods
            .iter_mut()
            .find(|m| m.name == "alreadyPurchased"),
        "couldn't find alreadyPurchased"
    );
    already_purchased.instructions = parse_fragment(RETURN_TRUE)?;
    Ok(())
}

fn main() -> Result<()> {
    let Args { project_path } = Args::parse();

    let smali_choice_path = project_path.join("smali/com/choiceofgames/choicescript");

    patch_class(
        &smali_choice_path.join("ChoiceScriptActivity.smali"),
        patch_main_activity,
    )?;

    patch_class(&smali_choice_path.join("Billing.smali"), patch_billing)?;

    let listjs = include_bytes!("../res/list.min.js");
    fs::write(project_path.join("assets/list.min.js"), listjs)?;

    let html_mod = include_str!("../res/mod.html");
    let index_path = project_path.join("assets/mygame/index.html");
    let html = fs::read_to_string(&index_path)?;
    fs::write(index_path, html.replace("</head>", html_mod))?;

    Ok(())
}
