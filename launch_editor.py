from nioh3_scroll_editor.updater import handle_update_command_line


if __name__ == "__main__":
    update_exit_code = handle_update_command_line()
    if update_exit_code is not None:
        raise SystemExit(update_exit_code)
    from nioh3_scroll_editor.app import main

    raise SystemExit(main())
